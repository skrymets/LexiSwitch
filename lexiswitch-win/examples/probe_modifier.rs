//! Does a modifier the user is still holding corrupt the chords this program sends?
//!
//! The hotkey is `Shift+Pause`. `RegisterHotKey` consumes the `Pause` keystroke, but nothing
//! releases `Shift` — the user is still holding it when the conversion starts, and the very
//! first thing a clipboard read does is send `Ctrl+C`. What the target actually receives is
//! `Ctrl+Shift+C`, or worse, if the synthetic `Ctrl` fails to register alongside a real
//! `Shift`, a plain `Shift+C` that types a capital C over the selection.
//!
//! That would explain both halves of a report that otherwise makes no sense: the copy produces
//! nothing *and* the selection disappears.
//!
//!   cargo run -p lexiswitch-win --example probe_modifier
//!
//! Runs the same copy twice against its own Notepad document — once cleanly, once with `Shift`
//! held down — and reports whether the copy worked and whether the document survived.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::process_name;
use lexiswitch_win::inject::{self, FocusGuard};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

const SEEDED: &str = "HELLO WORLD";
const SENTINEL: &str = "\u{1}probe-modifier-sentinel";

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

fn clipboard_text() -> Option<String> {
    Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?.text()
}

fn document() -> String {
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    let _ = inject::copy_selection();
    thread::sleep(Duration::from_millis(250));
    clipboard_text()
        .map(|t| t.trim_end_matches(['\r', '\n']).to_string())
        .unwrap_or_default()
}

/// Put the document back to a known state and select all of it.
fn reseed(guard: &FocusGuard) {
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    set_clipboard(SEEDED);
    let _ = inject::paste(guard);
    thread::sleep(Duration::from_millis(400));
    chord(VK_A);
    thread::sleep(Duration::from_millis(250));
}

/// One trial: plant a sentinel, send Ctrl+C (optionally with Shift held), report what happened.
fn trial(guard: &FocusGuard, label: &str, hold_shift: bool) {
    reseed(guard);
    set_clipboard(SENTINEL);
    thread::sleep(Duration::from_millis(150));

    if hold_shift {
        send(&[key(VK_SHIFT, false)]);
        thread::sleep(Duration::from_millis(50));
    }
    let _ = inject::copy_selection();
    thread::sleep(Duration::from_millis(300));
    if hold_shift {
        send(&[key(VK_SHIFT, true)]);
        thread::sleep(Duration::from_millis(50));
    }

    let copied = clipboard_text().unwrap_or_default();
    let copy_worked = copied == SEEDED;
    let sentinel_survived = copied == SENTINEL;

    // Reading the document uses the clipboard too, so this must come last.
    let doc = document();
    let intact = doc == SEEDED;

    say!("  {label}");
    say!(
        "     copy: {}",
        if copy_worked {
            "worked".to_string()
        } else if sentinel_survived {
            "NOTHING COPIED (sentinel survived)".to_string()
        } else {
            format!("unexpected {copied:?}")
        }
    );
    say!(
        "     document: {}",
        if intact {
            "intact".to_string()
        } else {
            format!("CHANGED -> {doc:?}")
        }
    );
}

fn main() {
    say!("probe_modifier — does a held Shift break the chords this program sends?");
    if std::process::Command::new("notepad.exe").spawn().is_err() {
        say!("could not launch notepad.exe");
        return;
    }
    for _ in 0..40 {
        // SAFETY: no preconditions; a null handle when nothing is focused is a valid answer.
        let hwnd = unsafe { GetForegroundWindow() };
        if process_name(hwnd).eq_ignore_ascii_case("notepad.exe") {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    thread::sleep(Duration::from_millis(1500));
    chord(VK_N);
    thread::sleep(Duration::from_millis(1200));

    let guard = FocusGuard::capture();
    say!();
    trial(&guard, "A  Ctrl+C with no modifier held (the control)", false);
    say!();
    trial(&guard, "B  Ctrl+C with Shift held, as after Shift+Pause", true);
    say!();
    say!("Close the Notepad window without saving.");
}
