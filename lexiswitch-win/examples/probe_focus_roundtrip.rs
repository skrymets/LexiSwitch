//! Does a selection survive the focus round-trip a chooser would impose?
//!
//! The pivot replaces immediate replacement with a keyboard-driven chooser. A chooser that
//! reads the keyboard must be focused, and the alternatives are closed: a non-activating
//! window plus a low-level hook is exactly what [ADR-0003](../../docs/adr/0003-raw-input-over-low-level-hook.md)
//! forbids, and registering `Up`/`Down`/`Enter`/`Esc` globally is not a real option. So the
//! chooser takes focus, and the entire interaction model rests on an assumption nobody has
//! measured: that the target application still has the same selection when focus comes back,
//! and will let us replace it.
//!
//!   cargo run -p lexiswitch-win --example probe_focus_roundtrip
//!   cargo run -p lexiswitch-win --example probe_focus_roundtrip -- --settle 600
//!   cargo run -p lexiswitch-win --example probe_focus_roundtrip -- "C:/Program Files/Notepad++/notepad++.exe"
//!
//! The stand-in window is a real top-level window owned by this process, not a borrowed one.
//! Whether `SetForegroundWindow` can hand focus back depends on *who is asking*, so a probe
//! that let some other application steal focus would be measuring a different question.
//!
//! Opens its own document in the target and refuses to type into anything else.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::{process_name, Uia};
use lexiswitch_win::inject::{self, FocusGuard, SYNTHETIC_MARKER};
use lexiswitch_win::selection;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    PeekMessageW, RegisterClassW, SetForegroundWindow, ShowWindow, TranslateMessage, MSG,
    PM_REMOVE, SW_SHOW, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

/// Seeded into the target, then selected.
const SOURCE: &str = "Original selected text for the focus round-trip probe";
/// Pasted after the round trip. Distinguishes replacement from insertion: if the selection
/// survived, this is the whole document; if it was lost, it landed at a caret instead.
const MARKER: &str = "REPLACED";

const CLASS_NAME: PCWSTR = w!("LexiSwitchRoundTripProbe");

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: SYNTHETIC_MARKER as usize,
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

/// A real top-level window, standing in for the chooser.
struct StandIn {
    hwnd: HWND,
}

impl StandIn {
    fn new() -> Option<Self> {
        // SAFETY: the class name is a static wide string with a NUL and the WNDCLASSW is fully
        // initialised before registration. A zero return from RegisterClassW can mean "already
        // registered", which is harmless; CreateWindowExW is the real check.
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(None).ok()?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let _ = RegisterClassW(&class);

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                w!("LexiSwitch chooser stand-in"),
                WS_OVERLAPPEDWINDOW,
                200,
                200,
                520,
                240,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .ok()?
        };
        Some(Self { hwnd })
    }

    /// Show and activate, then pump long enough for the activation to actually happen.
    fn take_foreground(&self) {
        // SAFETY: `hwnd` was created by this type and is still alive.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.pump(400);
    }

    /// Drain the message queue for `ms`, which a real chooser's loop would be doing anyway.
    fn pump(&self, ms: u64) {
        let steps = (ms / 20).max(1);
        for _ in 0..steps {
            let mut msg = MSG::default();
            // SAFETY: `msg` is a valid MSG for the whole call; PeekMessageW fills it or
            // reports that nothing was waiting.
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for StandIn {
    fn drop(&mut self) {
        // SAFETY: `hwnd` was created by this type and has not been destroyed elsewhere.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// `WNDPROC` requires `extern "system"` and windows-rs exposes `DefWindowProcW` as a plain
/// Rust `unsafe fn`, so it cannot be handed over directly.
///
/// SAFETY (contract on the caller, which is Windows): invoked only by the window manager for a
/// window of this class, with a valid `hwnd`. Nothing here dereferences `wp` or `lp`.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn set_clipboard(text: &str) -> Result<(), String> {
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).map_err(|e| e.to_string())?;
    clip.set_text(text).map_err(|e| e.to_string())
}

fn clipboard_text() -> Option<String> {
    Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?.text()
}

/// Read the whole document. Destroys the selection, so it must be the last thing done.
fn read_document() -> Option<String> {
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    inject::copy_selection().ok()?;
    thread::sleep(Duration::from_millis(250));
    clipboard_text().map(|t| t.trim_end_matches(['\r', '\n']).to_string())
}

/// Whether the target is showing an empty document.
///
/// Copying an empty selection leaves the clipboard untouched, so a sentinel is planted first
/// and surviving the copy is what proves nothing was selected.
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
        Some(text) => text == SENTINEL || text.trim().is_empty(),
        None => true,
    }
}

fn wait_for_target(process: &str) -> Option<HWND> {
    for _ in 0..40 {
        // SAFETY: no preconditions; a null handle is a valid answer.
        let hwnd = unsafe { GetForegroundWindow() };
        if !hwnd.is_invalid() && process_name(hwnd).eq_ignore_ascii_case(process) {
            return Some(hwnd);
        }
        thread::sleep(Duration::from_millis(250));
    }
    None
}

fn foreground() -> HWND {
    // SAFETY: no preconditions.
    unsafe { GetForegroundWindow() }
}

fn verdict(label: &str, ok: bool, detail: &str) -> bool {
    say!("  {}  {label:<44}{detail}", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() {
    let settle: u64 = std::env::args()
        .skip_while(|a| a != "--settle")
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let browse_ms: u64 = 1500;
    let target = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--") && a.parse::<u64>().is_err())
        .unwrap_or_else(|| "notepad.exe".to_string());
    let process = std::path::Path::new(&target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.clone());

    // `--attach` waits for an already-running target instead of launching one. Needed where
    // launching hands off to an existing instance — a browser opens a tab in the running
    // window rather than a new foreground process, so spawning would prove nothing.
    let attach = std::env::args().any(|a| a == "--attach");

    let uia = Uia::new();
    say!(
        "probe_focus_roundtrip — {} {target}",
        if attach { "attaching to" } else { "launching" }
    );
    say!("  browse pause {browse_ms} ms · settle after handback {settle} ms");
    if !attach && std::process::Command::new(&target).spawn().is_err() {
        say!("could not launch {target}");
        return;
    }
    let Some(_) = wait_for_target(&process) else {
        say!("{process} did not reach the foreground; nothing was typed.");
        return;
    };
    thread::sleep(Duration::from_millis(1500));

    // Never seed into whatever the target restored from its last session. `--no-new` is for
    // targets where Ctrl+N does not mean "new document" — in a browser it opens a window, and
    // the field under test is whatever the page focused.
    let no_new = std::env::args().any(|a| a == "--no-new");
    let mut empty = false;
    for _ in 0..3 {
        if !no_new {
            chord(VK_N);
            thread::sleep(Duration::from_millis(1200));
        }
        if document_is_empty() {
            empty = true;
            break;
        }
        if no_new {
            break;
        }
    }
    if !empty {
        say!("{process} is not showing an empty document. Nothing was typed into it.");
        return;
    }

    // Seed by pasting: typing 50 characters is the unreliable path this project is retiring.
    let guard = FocusGuard::capture();
    if set_clipboard(SOURCE).is_err() || inject::paste(&guard).is_err() {
        say!("could not seed the document.");
        return;
    }
    thread::sleep(Duration::from_millis(400));
    chord(VK_A);
    thread::sleep(Duration::from_millis(250));

    let target_hwnd = foreground();
    say!("  target window: {} ({})", target_hwnd.0 as isize, process_name(target_hwnd));
    say!();

    // Q4 — the read a chooser would take before it opens.
    let before = uia.as_ref().and_then(|u| {
        u.automation()
            .and_then(selection::read_via_automation)
    });
    say!(
        "  selection before focus moves: {}",
        match &before {
            Some(t) => format!("readable, {} chars", t.chars().count()),
            None => "no text pattern".to_string(),
        }
    );

    let Some(standin) = StandIn::new() else {
        say!("could not create the stand-in window.");
        return;
    };

    // Q2a — can this process take the foreground at all?
    standin.take_foreground();
    let took = foreground() == standin.hwnd;
    let mut ok = verdict(
        "stand-in took the foreground",
        took,
        &format!("foreground is {}", process_name(foreground())),
    );

    // The user browsing candidates.
    standin.pump(browse_ms);

    // Q2b — can it hand the foreground back?
    // SAFETY: no preconditions; returns false when the request is refused.
    unsafe {
        let _ = SetForegroundWindow(target_hwnd);
    }
    standin.pump(settle);
    let handed_back = foreground() == target_hwnd;
    ok &= verdict(
        "foreground handed back to target",
        handed_back,
        &format!("foreground is {}", process_name(foreground())),
    );

    if !handed_back {
        say!();
        say!("  Foreground could not be returned — the remaining checks would be meaningless.");
        return;
    }

    // Q1 / Q4 — is the same selection still there?
    let after = uia.as_ref().and_then(|u| {
        u.automation()
            .and_then(selection::read_via_automation)
    });
    match (&before, &after) {
        (Some(b), Some(a)) => {
            ok &= verdict(
                "selection readable and identical after",
                a == b,
                &format!("{} chars before, {} after", b.chars().count(), a.chars().count()),
            );
        }
        (Some(_), None) => {
            ok &= verdict("selection readable and identical after", false, "gone after refocus");
        }
        (None, _) => say!("  ----  selection comparison                     no text pattern; relying on the replacement test"),
    }

    // The test that actually matters: does a paste replace the selection, or land at a caret?
    let guard = FocusGuard::capture();
    if set_clipboard(MARKER).is_err() || inject::paste(&guard).is_err() {
        say!("  FAIL  replacement could not be attempted");
        return;
    }
    thread::sleep(Duration::from_millis(600));

    let doc = read_document();
    let replaced = doc.as_deref() == Some(MARKER);
    ok &= verdict(
        "paste replaced the selection",
        replaced,
        &match &doc {
            Some(d) if d == MARKER => "document is exactly the marker".to_string(),
            Some(d) => format!("document is {d:?}"),
            None => "document unreadable".to_string(),
        },
    );

    say!();
    say!(
        "{}",
        if ok {
            "ROUND TRIP SURVIVES — a focused chooser can replace the original selection."
        } else {
            "ROUND TRIP DOES NOT SURVIVE — the chooser must re-establish the range before replacing."
        }
    );
    say!("Close the {process} window without saving.");
}
