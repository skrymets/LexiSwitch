//! Hold a hotkey combination, standing in for another application that wants it.
//!
//! Three harnesses used to create a shortcut conflict by starting a second LexiSwitch. A second
//! LexiSwitch no longer starts, and the conflict state was never meant for one anyway — it
//! describes a combination held by a *different* application, so this is what should have been
//! holding it all along.
//!
//!   cargo run -p lexiswitch-win --example probe_hold_hotkey                 -- Shift+Pause
//!   cargo run -p lexiswitch-win --example probe_hold_hotkey -- "Ctrl+Alt+K" 30
//!
//! Registers the combination, prints whether it got it, and holds it until killed or until the
//! optional timeout in seconds elapses. It does nothing when the hotkey is pressed: the point is
//! only to own the registration.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::hotkey::{parse_binding, HotkeyId, Hotkeys};
use lexiswitch_win::window::MessageWindow;

fn main() {
    let mut args = std::env::args().skip(1);
    let binding_text = args.next().unwrap_or_else(|| "Shift+Pause".to_string());
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(600);

    let binding = match parse_binding(&binding_text) {
        Ok(b) => b,
        Err(e) => {
            say!("cannot parse {binding_text:?}: {e}");
            return;
        }
    };

    // A window of its own, so this holds the registration exactly as any other application
    // would — through its own message queue, not by borrowing LexiSwitch's.
    let window = match MessageWindow::new() {
        Ok(w) => w,
        Err(e) => {
            say!("could not create a window: {e}");
            return;
        }
    };

    let held = Hotkeys::register(window.hwnd(), &[(HotkeyId::ConvertSelection, binding)]);
    match held.failed.first() {
        None => {
            say!("HOLDING {binding_text} for {seconds}s — anything else asking for it will be refused");
            // The registration lives as long as `held`, which lives as long as this scope.
            thread::sleep(Duration::from_secs(seconds));
            say!("releasing {binding_text}");
        }
        Some(e) => say!("could not take {binding_text}: {e}"),
    }
}
