//! The chooser window, driven from the keyboard with no document in sight.
//!
//! Group 3's milestone is that the chooser is fully navigable from the keyboard with the mouse
//! unplugged. This drives it with synthesised keys and reports the decision that came back, so
//! navigation, expansion and the two commit chords are exercised without a transaction, a
//! selection, or anything that could modify a document if it went wrong.
//!
//!   cargo run -p lexiswitch-win --example probe_chooser              -- scripted
//!   cargo run -p lexiswitch-win --example probe_chooser -- --manual  -- drive it yourself
//!
//! The scripted run presses Down, Space (expand), Down (scroll), Space (collapse), Home, then
//! Enter — so a decision of `Apply { index: 0 }` proves that movement happened, that expansion
//! swallowed the arrows instead of moving the selection, and that Home came back to the top.
//!
//! Candidates come from the installed layouts, so what is on screen is real text in a real
//! script rather than ASCII that would hide a font problem.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_core::candidates;
use lexiswitch_win::chooser::{self, Decision};
use lexiswitch_win::layouts;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
};

/// Long enough that the compact preview has to elide, and typed under the wrong layout.
const SOURCE: &str = "Еуче шт Утпдшір — і це досить довгий рядок, щоб рядок довелося скоротити";

const VK_RETURN: u16 = 0x0D;
const VK_SPACE: u16 = 0x20;
const VK_HOME: u16 = 0x24;
const VK_DOWN: u16 = 0x28;

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn press(vk: u16) {
    let inputs = [key(vk, false), key(vk, true)];
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires.
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    thread::sleep(Duration::from_millis(320));
}

fn main() {
    let manual = std::env::args().any(|a| a == "--manual");

    let installed = match layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            say!("could not build layout tables: {e}");
            return;
        }
    };
    let tables: Vec<_> = installed.iter().map(|l| l.table.clone()).collect();
    // `--empty` shows the state where the selection read fine and no layout changes it. It is
    // a state of the chooser, not a refusal to open one, so it has to be looked at.
    let found = if std::env::args().any(|a| a == "--empty") {
        Vec::new()
    } else {
        candidates::generate(SOURCE, &tables, Some("en-US"))
    };

    say!("probe_chooser — {} candidates", found.len());
    for (i, c) in found.iter().enumerate() {
        say!(
            "  {i}  {} -> {}  unchanged={}  {:?}",
            c.from,
            c.to,
            c.unchanged,
            c.text.chars().take(28).collect::<String>()
        );
    }
    say!();

    if !manual {
        // The window has to exist before keys mean anything, and it is opened on this thread —
        // so the script runs on another one.
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(1400));
            say!("  sending Down          (move to candidate 1)");
            press(VK_DOWN);
            say!("  sending Space         (expand)");
            press(VK_SPACE);
            say!("  sending Down          (scroll, must NOT move the selection)");
            press(VK_DOWN);
            say!("  sending Space         (collapse)");
            press(VK_SPACE);
            say!("  sending Home          (back to candidate 0)");
            press(VK_HOME);
            say!("  sending Enter         (apply)");
            press(VK_RETURN);
        });
    } else {
        say!("  drive it yourself: arrows, Space, Enter, Ctrl+Enter, Esc");
    }

    match chooser::show(SOURCE, &found, Some("en-US")) {
        Ok(decision) => {
            say!();
            say!("  decision: {decision:?}");
            if !manual {
                let expected = Decision::Apply { index: 0 };
                if decision == expected {
                    say!("  PASS  navigation, expansion and commit all behaved");
                } else {
                    say!("  FAIL  expected {expected:?}");
                    if decision == Decision::Cancel {
                        say!("        a Cancel here usually means the window lost activation —");
                        say!("        check nothing else stole the foreground during the run");
                    }
                }
            }
        }
        Err(e) => say!("  FAIL  chooser did not open: {e}"),
    }
}
