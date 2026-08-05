//! Report what has keyboard focus and whether LexiSwitch can tell it is a password field.
//!
//! This is the instrument for task 7.4, which measures how far password-field detection
//! actually reaches. That measurement is the one the privacy model depends on, so it is
//! taken rather than assumed — run this, click into each application's password field, and
//! record what it says.
//!
//!   cargo run -p lexiswitch-win --example probe_focus
//!
//! Focus this window's *target*, not this console: the probe reports whatever has focus, so
//! leave it running in a corner and click around.

use std::{thread, time::Duration};

use lexiswitch_win::focus::{Detection, PasswordGate};

fn main() {
    let mut gate = PasswordGate::new();

    println!("probe_focus — reporting the focused control twice a second.");
    println!(
        "UI Automation: {}",
        if gate.has_automation() {
            "available"
        } else {
            "UNAVAILABLE — only the ES_PASSWORD window style can answer"
        }
    );
    println!();
    println!("{:<28} {:<12} {:<14} NOTE", "PROCESS", "PASSWORD?", "VIA");
    println!("{}", "-".repeat(78));

    let mut last = String::new();
    loop {
        // Focus can change without this process seeing an event, so the cache is dropped
        // each round — the point here is accuracy, not the message loop's latency budget.
        gate.invalidate();
        let info = gate.current();

        let note = match info.detection {
            Detection::UiAutomation => "",
            Detection::WindowStyle => "automation silent; style only — weak for non-Win32",
            Detection::Unknown => "NOTHING could answer — treated as not-a-password",
        };
        let line = format!(
            "{:<28} {:<12} {:<14} {}",
            if info.process.is_empty() {
                "<unknown>"
            } else {
                &info.process
            },
            if info.is_password { "YES" } else { "no" },
            match info.detection {
                Detection::UiAutomation => "UIAutomation",
                Detection::WindowStyle => "ES_PASSWORD",
                Detection::Unknown => "none",
            },
            note
        );

        // Only print on change, so the output is a log of what was observed rather than a
        // wall of repeats.
        if line != last {
            println!("{line}");
            last = line;
        }
        thread::sleep(Duration::from_millis(500));
    }
}
