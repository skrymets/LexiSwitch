//! Exercise the real selection-replacement path end to end and check what it promises.
//!
//! [ADR-0010](../../docs/adr/0010-paste-selection-replacement.md) makes three claims that are
//! worth failing loudly rather than trusting: the replacement arrives intact, the user's
//! clipboard comes back, and the whole conversion is a single undo step. This calls
//! `convert_selection` itself — not a re-implementation of it — so what passes here is the
//! shipped code path.
//!
//!   cargo run -p lexiswitch-win --example probe_paste_replacement
//!   cargo run -p lexiswitch-win --example probe_paste_replacement -- "C:/Program Files/Notepad++/notepad++.exe"
//!
//! Opens its own document in the target. Replaces the clipboard, and checks that it put it
//! back.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::{PasswordGate, Uia};
use lexiswitch_win::inject::{self, FocusGuard};
use lexiswitch_win::{layouts, selection};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N, VK_Z,
};

/// Typed with the Ukrainian layout active while intending English.
const SOURCE: &str = "Еуіе ша іудусеув еуче ьфн иу екфтідшеукфеув ифсл ещ еру сщккусе дщсфду";
const EXPECTED: &str = "Test if selected text may be transliterated back to the correct locale";

/// What the user's clipboard holds before the conversion, and must hold after it.
const PRIOR_CLIPBOARD: &str = "lexiswitch-prior-clipboard-value";

const UK: u16 = 0x0422;
const EN: u16 = 0x0409;

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: inject::SYNTHETIC_MARKER as usize,
            },
        },
    }
}

fn chord(vk: VIRTUAL_KEY) {
    let inputs = [
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ];
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires. Carries the synthetic marker, as everything this crate sends does.
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Put text into the target by pasting it.
///
/// Setup only, and deliberately not by typing it: seeding 70 characters through per-character
/// synthesis is exactly the unreliable mechanism under repair, and a mangled *setup* would be
/// scored as a failed conversion and send the reader after the wrong thing. Observed doing
/// precisely that on the first run of this probe.
fn seed_text(guard: &FocusGuard, text: &str) -> Result<(), String> {
    set_clipboard(text)?;
    inject::paste(guard).map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(400));
    Ok(())
}

fn read_document() -> Option<String> {
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    inject::copy_selection().ok()?;
    thread::sleep(Duration::from_millis(250));
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?;
    clip.text().map(|t| t.trim_end_matches(['\r', '\n']).to_string())
}

fn set_clipboard(text: &str) -> Result<(), String> {
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).map_err(|e| e.to_string())?;
    clip.set_text(text).map_err(|e| e.to_string())
}

fn clipboard_text() -> Option<String> {
    Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?.text()
}

/// Whether the target is showing an empty document.
///
/// The probe seeds by pasting, and an editor that restores its previous session comes up
/// holding the user's own files. Copying an empty selection leaves the clipboard untouched, so
/// a sentinel is planted first and surviving the copy is what proves nothing was selected.
fn document_is_empty() -> bool {
    const SENTINEL: &str = "\u{1}lexiswitch-empty-sentinel";
    if set_clipboard(SENTINEL).is_err() {
        return false;
    }
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    if inject::copy_selection().is_err() {
        return false;
    }
    thread::sleep(Duration::from_millis(250));
    match clipboard_text() {
        // Survived, so there was nothing to copy over it. Whitespace counts as empty: some
        // editors yield the blank line's terminator rather than nothing.
        Some(text) => text == SENTINEL || text.trim().is_empty(),
        None => true,
    }
}

fn wait_for_target(gate: &mut PasswordGate, process: &str) -> bool {
    for _ in 0..40 {
        gate.invalidate();
        if gate.current().process.eq_ignore_ascii_case(process) {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn check(label: &str, want: &str, got: Option<&str>) -> bool {
    match got {
        Some(g) if g == want => {
            say!("  PASS  {label}");
            true
        }
        Some(g) => {
            say!("  FAIL  {label}");
            say!("          want {want:?}");
            say!("          got  {g:?}");
            false
        }
        None => {
            say!("  FAIL  {label} — nothing could be read");
            false
        }
    }
}

fn main() {
    // Forces the clipboard read branch by withholding the automation interface, which
    // exercises the other half of `convert_selection` without needing a second editor whose
    // session this probe would have to disturb.
    let no_automation = std::env::args().any(|a| a == "--no-automation");
    let target = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .unwrap_or_else(|| "notepad.exe".to_string());
    let process = std::path::Path::new(&target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.clone());

    let installed = match layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            say!("could not build layout tables: {e}");
            return;
        }
    };
    let (Some(uk), Some(en)) = (
        installed.iter().find(|l| l.langid == UK),
        installed.iter().find(|l| l.langid == EN),
    ) else {
        say!("this probe needs both the Ukrainian and en-US layouts installed.");
        return;
    };

    let uia = if no_automation { None } else { Uia::new() };
    if uia.is_none() {
        say!("no automation interface — the clipboard read branch will be taken.");
    }
    let mut gate = PasswordGate::new();

    say!("probe_paste_replacement — launching {target}");
    if std::process::Command::new(&target).spawn().is_err() {
        say!("could not launch {target}");
        return;
    }
    if !wait_for_target(&mut gate, &process) {
        say!("{process} did not reach the foreground; nothing was typed.");
        return;
    }
    thread::sleep(Duration::from_millis(1500));

    // Never seed into whatever the target restored from its last session.
    let mut empty = false;
    for _ in 0..3 {
        chord(VK_N);
        thread::sleep(Duration::from_millis(1200));
        if document_is_empty() {
            empty = true;
            break;
        }
    }
    if !empty {
        say!("{process} is not showing an empty document. Nothing was pasted into it.");
        return;
    }

    // Seed the document with the mistyped text, then select it.
    let guard = FocusGuard::capture();
    if let Err(e) = seed_text(&guard, SOURCE) {
        say!("could not seed the document: {e}");
        return;
    }
    thread::sleep(Duration::from_millis(400));
    if read_document().as_deref() != Some(SOURCE) {
        say!("the seeded text did not arrive intact; the setup is unsound, not the conversion.");
        return;
    }

    // The user's clipboard, which must survive.
    if let Err(e) = set_clipboard(PRIOR_CLIPBOARD) {
        say!("could not set the prior clipboard: {e}");
        return;
    }
    chord(VK_A);
    thread::sleep(Duration::from_millis(200));

    say!("converting {} chars, uk -> en", SOURCE.chars().count());
    let outcome = selection::convert_selection(
        uia.as_ref().and_then(|u| u.automation()),
        HWND(std::ptr::null_mut()),
        &uk.table,
        &en.table,
        true,
        5,
    );

    let mut ok = true;
    match outcome {
        Err(e) => {
            say!("  FAIL  convert_selection returned an error: {e}");
            ok = false;
        }
        Ok(done) => {
            say!(
                "  read via {:?}, non-text clipboard content lost: {}",
                done.via,
                done.lost_nontext
            );
            thread::sleep(Duration::from_millis(600));
            // Clipboard first, always. `read_document` copies the document to the clipboard to
            // read it, so checking the restore afterwards measures the probe's own Ctrl+C
            // rather than the product's restore — which is what it did on the first run here.
            ok &= check(
                "prior clipboard restored",
                PRIOR_CLIPBOARD,
                clipboard_text().as_deref(),
            );
            ok &= check("replacement arrived intact", EXPECTED, read_document().as_deref());

            // One Ctrl+Z must return the whole thing, not one character of it.
            chord(VK_Z);
            thread::sleep(Duration::from_millis(500));
            ok &= check("one undo step reverts it all", SOURCE, read_document().as_deref());
        }
    }

    say!();
    say!("{}", if ok { "all checks passed" } else { "CHECKS FAILED" });
    say!("Close the {process} window without saving.");
}
