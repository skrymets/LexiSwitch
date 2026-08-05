//! Converting the current selection.
//!
//! Automation first, clipboard second ([ADR-0006](../../docs/adr/0006-uia-first-clipboard-fallback.md)).
//! Detection is by *attempt*: try the automation path, fall back on failure. There is no
//! list of known-good applications, because such a list is permanently out of date and fails
//! in exactly the direction that surprises the user.
//!
//! ## What each path actually does
//!
//! The two paths differ in how the selection is **read**, not how it is written.
//!
//! - **Automation** reads the selected text through UI Automation's text pattern. The
//!   clipboard is never touched, so clipboard managers and Windows Clipboard History never
//!   see the text.
//! - **Clipboard fallback** snapshots the clipboard, sends Ctrl+C, reads what landed there,
//!   and restores the snapshot afterwards.
//!
//! Both then replace the selection by synthesising the converted text, because typing over a
//! selection replaces it. UI Automation's text pattern is read-only — it can tell you what
//! is selected but not change it — so "automation replacement" is not a thing that exists,
//! and no amount of wanting makes it one.

use lexiswitch_core::layout::LayoutTable;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};

use crate::clipboard::Clipboard;
use crate::inject::{self, FocusGuard};
use crate::WinError;

/// How a selection was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadVia {
    /// UI Automation text pattern. The clipboard was not involved.
    Automation,
    /// Copy to the clipboard and back, with the previous contents restored.
    Clipboard,
}

/// Read the current selection through UI Automation.
///
/// Returns `None` when the focused element exposes no text pattern, which is the normal
/// case for terminals and many custom-drawn controls — that is the signal to fall back, not
/// an error worth reporting.
pub fn read_via_automation(automation: &IUIAutomation) -> Option<String> {
    // SAFETY: `automation` is a live interface. Every interface obtained here is a
    // windows-rs smart pointer released on drop, so no raw pointer escapes this function
    // and none crosses a thread boundary.
    unsafe {
        let element = automation.GetFocusedElement().ok()?;
        let pattern: IUIAutomationTextPattern =
            element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
        let ranges = pattern.GetSelection().ok()?;
        if ranges.Length().ok()? == 0 {
            return None;
        }
        let range = ranges.GetElement(0).ok()?;
        // -1 asks for the whole range rather than a truncated prefix.
        let text = range.GetText(-1).ok()?;
        let text = text.to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// Read the current selection by copying it, restoring the previous clipboard afterwards.
///
/// The previous **text** is restored; any other format is lost. On failure to acquire the
/// clipboard nothing is copied, nothing is pasted, and the clipboard is left alone.
pub fn read_via_clipboard(owner: HWND, attempts: u32) -> Result<ClipboardRead, SelectionError> {
    // Snapshot first: if this fails there is no point disturbing anything.
    let snapshot = {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        clip.text()
    };

    // Ctrl+C, then let the target act on it before reading.
    inject::copy_selection().map_err(SelectionError::Inject)?;
    std::thread::sleep(std::time::Duration::from_millis(60));

    let copied = {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        clip.text()
    };

    Ok(ClipboardRead {
        text: copied,
        previous: snapshot,
    })
}

/// The result of a clipboard read, carrying what must be put back.
pub struct ClipboardRead {
    pub text: Option<String>,
    pub previous: Option<String>,
}

impl ClipboardRead {
    /// Put the previous clipboard text back.
    ///
    /// Failure here is reported rather than swallowed: the user's clipboard is theirs, and
    /// silently keeping our copy of their selection on it is exactly the outcome ADR-0006
    /// promises not to leave behind.
    pub fn restore(&self, owner: HWND, attempts: u32) -> Result<(), SelectionError> {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        match &self.previous {
            Some(text) => clip.set_text(text).map_err(SelectionError::Win),
            // Nothing was there before. Leaving our copy would be worse than an empty
            // clipboard, so it is cleared.
            None => clip.set_text("").map_err(SelectionError::Win),
        }
    }
}

/// Convert the current selection in place.
///
/// `allow_clipboard` maps to the `allow_clipboard_fallback` setting: when false, an
/// application without a text pattern is reported as unsupported rather than being handled
/// through the clipboard.
pub fn convert_selection(
    automation: Option<&IUIAutomation>,
    owner: HWND,
    from: &LayoutTable,
    to: &LayoutTable,
    allow_clipboard: bool,
    attempts: u32,
) -> Result<Converted, SelectionError> {
    let guard = FocusGuard::capture();

    // Automation first — it touches nothing else.
    if let Some(automation) = automation {
        if let Some(text) = read_via_automation(automation) {
            let converted = lexiswitch_core::convert::convert(&text, from, to)
                .ok_or(SelectionError::NoChange)?;
            inject::replace(&guard, 0, &converted).map_err(SelectionError::Inject)?;
            return Ok(Converted {
                via: ReadVia::Automation,
                text: converted,
                clipboard_disturbed: false,
            });
        }
    }

    if !allow_clipboard {
        return Err(SelectionError::Unsupported);
    }

    let mut read = read_via_clipboard(owner, attempts)?;
    // Taken rather than moved out, so `read` stays whole and can still restore afterwards.
    let Some(text) = read.text.take() else {
        // Nothing was copied: there was probably no selection. Put the clipboard back and
        // report rather than pasting something arbitrary.
        let _ = read.restore(owner, attempts);
        return Err(SelectionError::NoSelection);
    };

    let converted =
        lexiswitch_core::convert::convert(&text, from, to).ok_or(SelectionError::NoChange)?;

    // Typing over a selection replaces it, so no backspaces are needed.
    let result = inject::replace(&guard, 0, &converted).map_err(SelectionError::Inject);
    // Restore regardless of whether the replacement worked — the clipboard must not keep
    // the user's selection either way.
    let restored = read.restore(owner, attempts);
    result?;
    restored?;

    Ok(Converted {
        via: ReadVia::Clipboard,
        text: converted,
        clipboard_disturbed: true,
    })
}

/// A successful selection conversion.
pub struct Converted {
    pub via: ReadVia,
    pub text: String,
    /// True when the clipboard was used, so any non-text content on it is now gone.
    pub clipboard_disturbed: bool,
}

/// Why a selection was not converted. Every variant names what did *not* happen.
#[derive(Debug)]
pub enum SelectionError {
    /// No text pattern, and the clipboard fallback is disabled by configuration.
    Unsupported,
    /// Nothing appears to be selected.
    NoSelection,
    /// Conversion would not change the text.
    NoChange,
    Busy(crate::clipboard::ClipboardBusy),
    Inject(inject::Abandoned),
    Win(WinError),
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(
                f,
                "this application does not expose its selection, and the clipboard fallback \
                 is switched off; the selection was not converted"
            ),
            Self::NoSelection => write!(f, "nothing is selected; nothing was changed"),
            Self::NoChange => write!(f, "converting would not change the selection"),
            Self::Busy(e) => write!(f, "{e}"),
            Self::Inject(e) => write!(f, "{e}"),
            Self::Win(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SelectionError {}
