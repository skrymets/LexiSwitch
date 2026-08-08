//! What modifiers are still down when a hotkey fires, and whether releasing them takes effect.
//!
//! Written for one question: `Alt+Win+Pause` reads a Notepad++ selection when Alt is pressed
//! first, and fails with "nothing is selected" when Win is pressed first. The hotkey fires
//! either way, so the difference is downstream — in `release_held_modifiers` or in the `Ctrl+C`
//! that follows it.
//!
//! This reports the async key state of every key that function inspects, before and after the
//! release, so the two orders can be compared against what the product actually sees rather
//! than against a theory about it.
//!
//!   cargo run -p lexiswitch-win --example probe_hotkey_modifiers -- "Alt+Win+Pause"
//!
//! Press the combination in one order, then the other. Ctrl-C to finish.

use std::io::Write;

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::hotkey::{parse_binding, HotkeyId, Hotkeys};
use lexiswitch_win::window::{Flow, MessageWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
    VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;

/// The keys `release_held_modifiers` inspects, plus Control, which it deliberately leaves alone.
const WATCHED: [(&str, VIRTUAL_KEY); 8] = [
    ("LShift", VK_LSHIFT),
    ("RShift", VK_RSHIFT),
    ("LAlt", VK_LMENU),
    ("RAlt", VK_RMENU),
    ("LWin", VK_LWIN),
    ("RWin", VK_RWIN),
    ("LCtrl", VK_LCONTROL),
    ("RCtrl", VK_RCONTROL),
];

fn down(vk: VIRTUAL_KEY) -> bool {
    // SAFETY: no preconditions; reports whether one key is currently down. The high bit is the
    // pressed state, which is the sign bit of the i16 returned.
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn snapshot() -> String {
    WATCHED
        .iter()
        .filter(|(_, vk)| down(*vk))
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join("+")
}

/// Release exactly what the product releases, the same way it does.
fn release_like_the_product() {
    const HELD: [VIRTUAL_KEY; 6] = [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];
    let ups: Vec<INPUT> = HELD
        .into_iter()
        .filter(|vk| down(*vk))
        .map(|vk| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        })
        .collect();
    if ups.is_empty() {
        return;
    }
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires.
    unsafe {
        SendInput(&ups, std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(std::time::Duration::from_millis(30));
}

fn main() {
    let binding_text = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Alt+Win+Pause".to_string());
    let binding = match parse_binding(&binding_text) {
        Ok(b) => b,
        Err(e) => {
            say!("cannot parse {binding_text:?}: {e}");
            return;
        }
    };

    let window = match MessageWindow::new() {
        Ok(w) => w,
        Err(e) => {
            say!("could not create a window: {e}");
            return;
        }
    };
    let registered = Hotkeys::register(window.hwnd(), &[(HotkeyId::ChangeCase, binding)]);
    if let Some(failure) = registered.failed.first() {
        say!("could not take {binding_text}: {failure}");
        return;
    }
    say!("holding {binding_text}. Press it one way, then the other. Ctrl-C to stop.");

    let mut press = 0;
    let _ = window.run(|msg| {
        if msg.message == WM_HOTKEY {
            press += 1;
            let before = snapshot();
            release_like_the_product();
            let after = snapshot();
            say!("#{press}  held at fire: [{before}]   after release: [{after}]");
        }
        Flow::Continue
    });
}
