//! The clipboard, used only as a fallback for reading a selection.
//!
//! Per [ADR-0006](../../docs/adr/0006-uia-first-clipboard-fallback.md) this is the second
//! choice: the automation path reads a selection without touching the clipboard at all, and
//! this exists for applications that do not expose one. Using it puts the user's selected
//! text on the clipboard briefly, where clipboard managers and Windows Clipboard History can
//! see it — inherent to the mechanism, identical to pressing Ctrl+C, and disableable in
//! configuration.
//!
//! **Content this program places can be kept out of Clipboard History.** Windows exposes
//! opt-out formats for History, Cloud Clipboard and third-party monitors, and
//! [`Clipboard::set_text_excluded`] sets them. They apply only to data written here — a
//! selection that the target application copied in response to a synthesised Ctrl+C is placed
//! by that application, and is in History before this process can read it.
//!
//! **Only text is preserved.** Snapshotting and restoring covers `CF_UNICODETEXT`. Anything
//! else on the clipboard — an image, rich text, a file list — is destroyed by the round trip
//! and not restored. That is deliberate: a partial reconstruction that looks successful is
//! worse than an honest text-only restore, because nobody re-checks a clipboard that appears
//! to have survived. Callers should say so when it happens.

use std::sync::OnceLock;
use std::{thread, time::Duration};

use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_LOCALE, CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT};

use crate::WinError;

const CF_TEXT_U32: u32 = CF_UNICODETEXT.0 as u32;

/// The formats that opt clipboard content out of Windows Clipboard History, Cloud Clipboard,
/// and third-party clipboard monitors.
///
/// Registered once and cached: the atom is per-session, and every process asking for a given
/// name gets the same number back. A name that fails to register is dropped rather than
/// recorded as zero, so a caller never asks the clipboard to store data under format 0.
///
/// These cover content *this program* places. They cannot protect a selection that the target
/// application copied in response to a synthesised Ctrl+C — that data is placed by the target,
/// under its own call, and is in History before this process ever reads it.
fn history_exclusion_formats() -> &'static [u32] {
    static FORMATS: OnceLock<Vec<u32>> = OnceLock::new();
    FORMATS.get_or_init(|| {
        // SAFETY: each argument is a NUL-terminated wide string literal with 'static lifetime,
        // which is exactly what RegisterClipboardFormatW reads. It returns 0 on failure and
        // has no other failure mode; zero is filtered out below.
        let ids = unsafe {
            [
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
                RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
                RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
            ]
        };
        ids.into_iter().filter(|id| *id != 0).collect()
    })
}

/// Place a single zero `DWORD` under `format`.
///
/// All three exclusion formats are read as a boolean opt-out, and zero is the opt-out value.
/// Requires the clipboard to be open and already emptied.
fn set_zero_dword(format: u32) -> Result<(), WinError> {
    // SAFETY: a moveable global block of exactly four bytes, written once through a locked
    // pointer and unlocked before use. Ownership passes to the clipboard on a successful
    // SetClipboardData, so it is not freed here; on failure it leaks four bytes rather than
    // risking a double free of a handle the clipboard may already own.
    unsafe {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>())
            .map_err(|e| WinError::new("GlobalAlloc", e))?;
        let dst = GlobalLock(hglobal) as *mut u32;
        if dst.is_null() {
            return Err(WinError::last("GlobalLock"));
        }
        *dst = 0;
        let _ = GlobalUnlock(hglobal);

        SetClipboardData(format, Some(HANDLE(hglobal.0)))
            .map_err(|e| WinError::new("SetClipboardData", e))?;
    }
    Ok(())
}

/// An open clipboard, closed when dropped.
///
/// The clipboard is a single global resource that any process can hold. Acquiring it can
/// legitimately fail because something else is mid-operation, so opening retries with
/// backoff rather than failing at the first refusal.
pub struct Clipboard {
    _private: (),
}

impl Clipboard {
    /// Open the clipboard, retrying with backoff.
    ///
    /// Total wait is bounded: with the default of 5 attempts the delays are 10, 20, 40 and
    /// 80 ms, so a busy clipboard costs 150 ms rather than an unbounded stall.
    pub fn open(owner: HWND, attempts: u32) -> Result<Self, ClipboardBusy> {
        let mut delay = 10u64;
        let mut last: Option<WinError> = None;

        for attempt in 0..attempts.max(1) {
            // SAFETY: `owner` is a window belonging to this thread, which is what
            // OpenClipboard associates ownership with.
            let opened = unsafe { OpenClipboard(Some(owner)) };
            match opened {
                Ok(()) => return Ok(Self { _private: () }),
                Err(e) => {
                    last = Some(WinError::new("OpenClipboard", e));
                    if attempt + 1 < attempts {
                        thread::sleep(Duration::from_millis(delay));
                        delay *= 2;
                    }
                }
            }
        }
        Err(ClipboardBusy {
            attempts,
            last: last.expect("at least one attempt"),
        })
    }

    /// The clipboard's text, or `None` when it holds no text.
    pub fn text(&self) -> Option<String> {
        // SAFETY: the clipboard is open for the lifetime of `self`. The handle returned by
        // GetClipboardData is owned by the clipboard and must not be freed here; it stays
        // valid until the clipboard is closed, which Drop does after this returns.
        unsafe {
            if IsClipboardFormatAvailable(CF_TEXT_U32).is_err() {
                return None;
            }
            let handle: HANDLE = GetClipboardData(CF_TEXT_U32).ok()?;
            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                return None;
            }
            // Walk to the NUL terminator; clipboard text is always NUL-terminated.
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(hglobal);
            Some(text)
        }
    }

    /// Whether the clipboard holds anything that is not text.
    ///
    /// Answers the only question worth asking the user about: a round trip preserves text and
    /// destroys everything else, so this is what distinguishes "your clipboard survived" from
    /// "your screenshot did not".
    ///
    /// A format *count* cannot answer it. Putting text on the clipboard causes Windows to
    /// synthesise companions — `CF_TEXT`, `CF_OEMTEXT`, `CF_LOCALE` — so plain text routinely
    /// reports four formats. The list has to be walked and the text-ish ones discounted.
    pub fn holds_nontext(&self) -> bool {
        const TEXTISH: [u32; 4] = [
            CF_TEXT.0 as u32,
            CF_OEMTEXT.0 as u32,
            CF_UNICODETEXT.0 as u32,
            CF_LOCALE.0 as u32,
        ];

        // SAFETY: the clipboard is open for the lifetime of `self`. EnumClipboardFormats walks
        // the format list from the value handed back each time and returns 0 at the end; it
        // borrows nothing that outlives the call.
        unsafe {
            let mut format = EnumClipboardFormats(0);
            while format != 0 {
                if !TEXTISH.contains(&format) {
                    return true;
                }
                format = EnumClipboardFormats(format);
            }
        }
        false
    }

    /// Replace the clipboard contents with `text`.
    ///
    /// Everything previously on the clipboard, in every format, is discarded — see the
    /// module note.
    ///
    /// The text is left visible to Clipboard History, which is correct for the one thing this
    /// is used for: putting back what the user had. That content is theirs and was already in
    /// their history when they copied it; suppressing it on the way back would quietly change
    /// the behaviour of their own data. Use [`Self::set_text_excluded`] for text this program
    /// generated.
    pub fn set_text(&self, text: &str) -> Result<(), WinError> {
        self.write_text(text, false)
    }

    /// Replace the clipboard contents with `text`, kept out of Clipboard History.
    ///
    /// For text this program derived from what the user typed. Clipboard History persists
    /// entries and Cloud Clipboard can sync them off the machine, so keystroke-derived content
    /// placed without these formats is a route to disk.
    ///
    /// If the exclusion cannot be applied the clipboard is left **empty** and the error is
    /// returned — the text is never placed unprotected. An empty clipboard is a visible,
    /// recoverable annoyance; a silent leak is neither.
    pub fn set_text_excluded(&self, text: &str) -> Result<(), WinError> {
        self.write_text(text, true)
    }

    fn write_text(&self, text: &str, exclude_from_history: bool) -> Result<(), WinError> {
        let mut utf16: Vec<u16> = text.encode_utf16().collect();
        utf16.push(0);
        let bytes = std::mem::size_of_val(&utf16[..]);

        // SAFETY: a moveable global block of exactly the size written below. Ownership of
        // the block passes to the clipboard on a successful SetClipboardData, so it is not
        // freed here; on failure it leaks one small allocation rather than risking a
        // double free of a handle the clipboard may already own.
        unsafe {
            let hglobal =
                GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| WinError::new("GlobalAlloc", e))?;
            let dst = GlobalLock(hglobal) as *mut u16;
            if dst.is_null() {
                return Err(WinError::last("GlobalLock"));
            }
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
            let _ = GlobalUnlock(hglobal);

            EmptyClipboard().map_err(|e| WinError::new("EmptyClipboard", e))?;

            // Before the text, deliberately. A failure here returns with the clipboard empty
            // rather than holding unprotected content.
            if exclude_from_history {
                for format in history_exclusion_formats() {
                    set_zero_dword(*format)?;
                }
            }

            SetClipboardData(CF_TEXT_U32, Some(HANDLE(hglobal.0)))
                .map_err(|e| WinError::new("SetClipboardData", e))?;
        }
        Ok(())
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: closes the clipboard this type opened, exactly once.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// The clipboard could not be acquired.
#[derive(Debug)]
pub struct ClipboardBusy {
    pub attempts: u32,
    pub last: WinError,
}

impl std::fmt::Display for ClipboardBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "clipboard busy after {} attempts ({}); nothing was pasted and the clipboard is \
             unchanged",
            self.attempts,
            self.last.message()
        )
    }
}

impl std::error::Error for ClipboardBusy {}
