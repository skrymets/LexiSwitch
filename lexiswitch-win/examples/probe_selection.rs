//! Report whether the focused control exposes its selection to UI Automation.
//!
//! This is the instrument for the automation-versus-clipboard fork in
//! `selection::convert_selection`: the automation path is taken only when the focused element
//! yields a text pattern *and* a non-empty selection, and everything else falls back to the
//! clipboard. When a conversion silently changes nothing, the first question is which of those
//! two paths ran. Guessing from the application's name is exactly the allowlist thinking
//! [ADR-0006](../../docs/adr/0006-uia-first-clipboard-fallback.md) rejects, so it is measured.
//!
//!   cargo run -p lexiswitch-win --example probe_selection
//!
//! Select some text in the target application and read the line that appears. Focus the
//! target, not this console.
//!
//! Selection *lengths* are reported, never the selected text.

use std::{thread, time::Duration};

use lexiswitch_win::focus::{process_name, Uia};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::Win32::UI::Accessibility::{IUIAutomationTextPattern, UIA_TextPatternId};

/// How far the automation read got before giving up.
///
/// `read_via_automation` collapses every one of these into `None`, which is right for the
/// program and useless for diagnosis — hence the separate ladder here.
enum Reach {
    NoAutomation,
    NoFocusedElement,
    NoTextPattern,
    NoSelectionCall,
    EmptySelection,
    Text(usize),
}

impl Reach {
    /// Which path `convert_selection` would take given this result.
    fn path(&self) -> &'static str {
        match self {
            Self::Text(_) => "AUTOMATION",
            _ => "clipboard",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::NoAutomation => "UI Automation unavailable".into(),
            Self::NoFocusedElement => "GetFocusedElement failed".into(),
            Self::NoTextPattern => "no text pattern on the focused element".into(),
            Self::NoSelectionCall => "text pattern present, GetSelection failed".into(),
            Self::EmptySelection => "text pattern present, nothing readable selected".into(),
            Self::Text(n) => format!("selection readable, len={n}"),
        }
    }
}

fn probe(uia: Option<&Uia>) -> Reach {
    let Some(automation) = uia.and_then(|u| u.automation()) else {
        return Reach::NoAutomation;
    };

    // SAFETY: `automation` is a live interface borrowed from the gate, which created it on
    // this thread's apartment and outlives this call. Every interface obtained here is a
    // windows-rs smart pointer released on drop, so no raw pointer escapes this function and
    // none crosses a thread boundary. The call sequence mirrors `read_via_automation`.
    unsafe {
        let Ok(element) = automation.GetFocusedElement() else {
            return Reach::NoFocusedElement;
        };
        let pattern: IUIAutomationTextPattern = match element.GetCurrentPatternAs(UIA_TextPatternId)
        {
            Ok(pattern) => pattern,
            Err(_) => return Reach::NoTextPattern,
        };
        let Ok(ranges) = pattern.GetSelection() else {
            return Reach::NoSelectionCall;
        };
        if !matches!(ranges.Length(), Ok(n) if n > 0) {
            return Reach::EmptySelection;
        }
        let Ok(range) = ranges.GetElement(0) else {
            return Reach::EmptySelection;
        };
        // -1 asks for the whole range rather than a truncated prefix, as the real read does.
        let Ok(text) = range.GetText(-1) else {
            return Reach::EmptySelection;
        };
        match text.to_string().chars().count() {
            0 => Reach::EmptySelection,
            len => Reach::Text(len),
        }
    }
}

fn main() {
    let uia = Uia::new();

    println!("probe_selection — the path convert_selection would take, twice a second.");
    println!("Select text in the target application. Focus the target, not this console.");
    println!();
    println!("{:<28} {:<12} WHY", "PROCESS", "PATH");
    println!("{}", "-".repeat(78));

    let mut last = String::new();
    loop {
        // SAFETY: no preconditions; a null handle when nothing is focused is a valid answer.
        let process = process_name(unsafe { GetForegroundWindow() });
        let reach = probe(uia.as_ref());

        let line = format!(
            "{:<28} {:<12} {}",
            if process.is_empty() {
                "<unknown>"
            } else {
                &process
            },
            reach.path(),
            reach.detail()
        );

        // Only print on change, so the output is a log of what was observed rather than a wall
        // of repeats.
        if line != last {
            println!("{line}");
            last = line;
        }
        thread::sleep(Duration::from_millis(500));
    }
}
