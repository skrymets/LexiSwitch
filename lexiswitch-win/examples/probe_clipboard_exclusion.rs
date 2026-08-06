//! Confirm that clipboard content this program places can be kept out of Clipboard History.
//!
//! `Clipboard::set_text_excluded` claims to opt its content out of Windows Clipboard History,
//! Cloud Clipboard and third-party monitors. That claim is a privacy guarantee, so it is
//! measured rather than assumed: the formats are checked for presence after the write, and the
//! plain `set_text` is checked for their absence in the same run. One without the other proves
//! nothing — a probe that only ever sees the formats present cannot tell a working exclusion
//! from a stuck one.
//!
//!   cargo run -p lexiswitch-win --example probe_clipboard_exclusion
//!
//! Replaces the clipboard contents. Nothing is read from the clipboard beyond what it writes.

use lexiswitch_win::clipboard::Clipboard;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW};

const MARKER: &str = "lexiswitch-exclusion-probe";

fn formats() -> Vec<(&'static str, u32)> {
    // SAFETY: each argument is a NUL-terminated wide string literal with 'static lifetime,
    // which is what RegisterClipboardFormatW reads. It returns 0 on failure and nothing else.
    unsafe {
        vec![
            (
                "ExcludeClipboardContentFromMonitorProcessing",
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            ),
            (
                "CanIncludeInClipboardHistory",
                RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
            ),
            (
                "CanUploadToCloudClipboard",
                RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
            ),
        ]
    }
}

/// Which of the exclusion formats are currently on the clipboard.
///
/// Read with the clipboard closed: `IsClipboardFormatAvailable` does not require it open, and
/// checking through a fresh look is closer to what a clipboard monitor actually does.
fn present(formats: &[(&'static str, u32)]) -> Vec<&'static str> {
    formats
        .iter()
        .filter(|(_, id)| {
            // SAFETY: no preconditions beyond a registered format number, and a failure return
            // simply means the format is not on the clipboard.
            *id != 0 && unsafe { IsClipboardFormatAvailable(*id) }.is_ok()
        })
        .map(|(name, _)| *name)
        .collect()
}

fn write(excluded: bool) -> Result<(), String> {
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).map_err(|e| e.to_string())?;
    let result = if excluded {
        clip.set_text_excluded(MARKER)
    } else {
        clip.set_text(MARKER)
    };
    result.map_err(|e| e.to_string())
}

fn read_back() -> Option<String> {
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?;
    clip.text()
}

fn main() {
    let formats = formats();

    println!("registered formats:");
    for (name, id) in &formats {
        println!("  {id:>6}  {name}");
    }
    if formats.iter().any(|(_, id)| *id == 0) {
        println!("at least one format failed to register — exclusion cannot be relied on.");
        return;
    }
    println!();

    for (label, excluded, want) in [
        ("set_text (user's own content)", false, 0usize),
        ("set_text_excluded (generated)", true, formats.len()),
    ] {
        match write(excluded) {
            Err(e) => println!("{label:<32} FAILED to write: {e}"),
            Ok(()) => {
                let on_clipboard = present(&formats);
                let text_ok = read_back().as_deref() == Some(MARKER);
                let verdict = if on_clipboard.len() == want && text_ok {
                    "as specified"
                } else {
                    "NOT AS SPECIFIED"
                };
                println!(
                    "{label:<32} {} of {} exclusion formats, text {} — {verdict}",
                    on_clipboard.len(),
                    formats.len(),
                    if text_ok { "intact" } else { "WRONG" }
                );
                for name in on_clipboard {
                    println!("{:>34}{name}", "");
                }
            }
        }
    }

    println!();
    println!("The clipboard now holds {MARKER:?}.");
}
