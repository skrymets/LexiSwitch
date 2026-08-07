//! Is "somebody opened the clipboard" evidence that a paste happened?
//!
//! The replacement path waits for a consumer to take the clipboard before restoring it, which
//! fixed a real race in Chrome. The tempting next step is to treat that same signal as proof
//! the paste landed — a target that never read the clipboard cannot have pasted from it — and
//! to report a failure when it does not appear.
//!
//! That only works if nothing *else* reads the clipboard. Windows Clipboard History, cloud
//! clipboard and third-party managers all read whatever appears there, and any of them would
//! make the signal fire whether or not a paste occurred.
//!
//!   cargo run -p lexiswitch-win --example probe_clipboard_consumer
//!
//! Writes to the clipboard and waits, sending no paste at all. A consumer observed here is a
//! consumer that was never asked for, and the signal is worthless for confirming anything.

use std::io::Write;
use std::time::Duration;

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::clipboard::{self, Clipboard};
use windows::Win32::Foundation::HWND;

fn main() {
    let owner = HWND(std::ptr::null_mut());

    for (label, excluded) in [
        ("plain text", false),
        ("with the history-exclusion formats set", true),
    ] {
        let Ok(clip) = Clipboard::open(owner, 5) else {
            say!("could not open the clipboard");
            return;
        };
        let wrote = if excluded {
            clip.set_text_excluded("lexiswitch-consumer-probe")
        } else {
            clip.set_text("lexiswitch-consumer-probe")
        };
        drop(clip);
        if wrote.is_err() {
            say!("could not write the clipboard");
            return;
        }

        // No paste is sent. Anything that opens the clipboard now did so of its own accord.
        let start = std::time::Instant::now();
        let seen = clipboard::wait_for_consumer(Duration::from_millis(2000));
        say!(
            "  {label:<40} consumer seen: {seen:<5}  after {:?}",
            start.elapsed()
        );
    }

    say!();
    say!("A consumer seen here means the signal cannot confirm a paste: something on this");
    say!("machine reads the clipboard whenever it changes, paste or no paste.");
}
