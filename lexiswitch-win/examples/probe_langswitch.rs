//! Does asking another application to change its input language actually change it?
//!
//! `Ctrl+Enter` converts and then requests the candidate's target layout. The conversion lands
//! and the layout does not change, so either the request is never made or the target declines
//! it. This separates the two: it asks directly and then reads back what the foreground window's
//! input language actually is.
//!
//!   cargo run -p lexiswitch-win --example probe_langswitch
//!
//! `WM_INPUTLANGCHANGEREQUEST` is advisory by design — an application is free to ignore it and
//! there is no return value that says it did. So the only way to know is to look afterwards,
//! which is what this does, against a real window rather than our own.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::focus::process_name;
use lexiswitch_win::langswitch;
use lexiswitch_win::layouts;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

fn main() {
    let installed = match layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            say!("could not build layout tables: {e}");
            return;
        }
    };

    say!("probe_langswitch — installed layouts:");
    for l in &installed {
        say!(
            "  {:<8} langid=0x{:04X}  hkl=0x{:016X}",
            l.name,
            l.langid,
            l.hkl
        );
    }

    // SAFETY: no preconditions; a null handle is a valid answer.
    let hwnd = unsafe { GetForegroundWindow() };
    say!();
    say!("foreground: {}", process_name(hwnd));
    say!("langid before: 0x{:04X}", layouts::foreground_langid());

    // A named layout when one is given, so the odd handles can be tested individually — not
    // every installed layout has the tidy 0xLLLLLLLL shape, and one that does is no evidence
    // about one that does not.
    let wanted = std::env::args().nth(1);
    let current = layouts::foreground_langid();
    let target = match wanted.as_deref() {
        Some(name) => installed.iter().find(|l| l.name == name),
        None => installed.iter().find(|l| l.langid != current),
    };
    let Some(target) = target else {
        say!("no such layout to switch to");
        return;
    };
    say!();
    say!("requesting {} (hkl=0x{:016X})", target.name, target.hkl);

    match langswitch::activate_for_foreground(target.hkl) {
        Ok(()) => say!("  the request was posted"),
        Err(e) => {
            say!("  the request could not be posted: {e}");
            return;
        }
    }

    // Read back repeatedly: the request is posted, so the target acts on it when it next pumps
    // its queue, not before this line runs.
    for ms in [100u64, 300, 600, 1200, 2400] {
        thread::sleep(Duration::from_millis(ms));
        let now = layouts::foreground_langid();
        say!(
            "  after {:>4} ms more: langid=0x{:04X}{}",
            ms,
            now,
            if now == target.langid { "   CHANGED" } else { "" }
        );
        if now == target.langid {
            say!();
            say!("RESULT: the request works against {}", process_name(hwnd));
            return;
        }
    }

    say!();
    say!(
        "RESULT: {} ignored the request — the layout did not change",
        process_name(hwnd)
    );
}
