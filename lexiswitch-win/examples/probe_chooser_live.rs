//! The chooser on the real path: a real selection, in a real application, read through a real
//! transaction.
//!
//! `probe_chooser` shows the window with synthetic candidates and nothing else going on, and it
//! passes. The reported defect — the chooser appearing for a fraction of a second and vanishing
//! — happens only through the whole path, so this reproduces the whole path: launch Notepad,
//! seed text typed under the wrong layout, select it, read it into a transaction, and open the
//! chooser exactly as the worker does.
//!
//!   cargo run -p lexiswitch-win --features chooser-diag --example probe_chooser_live
//!   cargo run -p lexiswitch-win --features chooser-diag --example probe_chooser_live -- --shift
//!
//! `--shift` holds Shift down across the whole transaction, as it is when the user has just
//! pressed Shift+Pause and has not let go. `RegisterHotKey` consumes the Pause, but nothing
//! releases the Shift.
//!
//! Opens its own document and refuses to type into anything else.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_core::candidates;
use lexiswitch_win::chooser;
use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::{process_name, Uia};
use lexiswitch_win::inject::{self, FocusGuard};
use lexiswitch_win::layouts;
use lexiswitch_win::transaction::Transaction;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// Exactly the user's reported text: typed with en-US active while meaning Ukrainian.
const SOURCE: &str = "Ghbdsn/ \" gbnfyyz ojlj ndj'] ghtptynfws]";

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires.
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn chord(vk: VIRTUAL_KEY) {
    send(&[
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ]);
}

fn set_clipboard(text: &str) -> bool {
    Clipboard::open(HWND(std::ptr::null_mut()), 5)
        .ok()
        .and_then(|c| c.set_text(text).ok())
        .is_some()
}

fn main() {
    let hold_shift = std::env::args().any(|a| a == "--shift");
    // `--here` acts on whatever is already focused and already selected, launching nothing and
    // seeding nothing. The reported Notepad++ failure is in an existing document, and a probe
    // that opens its own can only ever reproduce a probe opening its own.
    let here = std::env::args().any(|a| a == "--here");
    say!(
        "probe_chooser_live — the real path{}",
        if hold_shift { ", with Shift held" } else { "" }
    );

    let installed = match layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            say!("could not build layout tables: {e}");
            return;
        }
    };
    let tables: Vec<_> = installed.iter().map(|l| l.table.clone()).collect();
    let uia = Uia::new();

    if !here {
        if std::process::Command::new("notepad.exe").spawn().is_err() {
            say!("could not launch notepad.exe");
            return;
        }
        for _ in 0..40 {
            // SAFETY: no preconditions; a null handle is a valid answer.
            if process_name(unsafe { GetForegroundWindow() }).eq_ignore_ascii_case("notepad.exe") {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }
        thread::sleep(Duration::from_millis(1500));
        chord(VK_N);
        thread::sleep(Duration::from_millis(1200));

        // Seed by pasting rather than typing: typing is the mechanism this project retired.
        let guard = FocusGuard::capture();
        if !set_clipboard(SOURCE) || inject::paste(&guard).is_err() {
            say!("could not seed the document");
            return;
        }
        thread::sleep(Duration::from_millis(500));
        chord(VK_A);
        thread::sleep(Duration::from_millis(300));
    }

    if hold_shift {
        say!("  holding Shift down, as after Shift+Pause");
        send(&[key(VK_SHIFT, false)]);
        thread::sleep(Duration::from_millis(80));
    }

    // SAFETY: no preconditions.
    let before = unsafe { GetForegroundWindow() };
    say!("  foreground before begin: {}", process_name(before));

    let transaction = match Transaction::begin(
        uia.as_ref().and_then(|u| u.automation()),
        HWND(std::ptr::null_mut()),
        true,
        5,
    ) {
        Ok(t) => t,
        Err(e) => {
            say!("  FAIL  begin: {e}");
            if hold_shift {
                send(&[key(VK_SHIFT, true)]);
            }
            return;
        }
    };
    say!(
        "  read {:?} via {:?}",
        transaction.original().chars().take(24).collect::<String>(),
        transaction.via()
    );

    let found = candidates::generate(transaction.original(), &tables, Some("en-US"));
    say!("  {} candidates", found.len());
    if found.is_empty() {
        say!("  nothing to choose between");
        let _ = transaction.cancel();
        if hold_shift {
            send(&[key(VK_SHIFT, true)]);
        }
        return;
    }

    say!("  opening the chooser — leave the mouse and keyboard alone");
    let decision = chooser::show(
        transaction.original(),
        &found,
        Some("en-US"),
        10,
        windows::Win32::Foundation::HWND(std::ptr::null_mut()),
        5,
    )
    .map(|(decision, _copied)| decision);

    if hold_shift {
        send(&[key(VK_SHIFT, true)]);
    }

    match decision {
        Ok(d) => say!("  decision: {d:?}"),
        Err(e) => say!("  chooser error: {e}"),
    }
    let _ = transaction.cancel();
    say!("  (cancelled; the document is unchanged. Close Notepad without saving.)");
}
