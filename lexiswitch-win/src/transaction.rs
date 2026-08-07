//! A conversion in progress: read now, decide later, replace only on commit.
//!
//! The old shape did everything in one call — read the selection, convert it, and paste the
//! result before returning. That is what made success, no-change and failure indistinguishable
//! from the outside, and it is what the pivot removes
//! ([ADR-0012](../../docs/adr/0012-single-chooser-transaction.md)).
//!
//! A transaction splits that in two. [`Transaction::begin`] reads the selection **while the
//! user's application still has focus**, before any chooser exists to take it away, and
//! snapshots everything that will be needed to put the world back. Nothing is modified. The
//! caller then takes as long as it likes — showing candidates, waiting for arrow keys — and
//! ends with [`Transaction::commit`] or [`Transaction::cancel`], both of which restore the
//! foreground to the application the text came from.
//!
//! ## Why reading happens first
//!
//! A chooser must be focused to receive the keyboard, which means the application being edited
//! loses focus while the user decides. Reading a selection out of a window that no longer has
//! focus is a strictly harder problem for no benefit, so the read is taken up front and the
//! text is carried across.
//!
//! That the selection *survives* the round trip is measured, not assumed — across a Win32
//! control with a text pattern, a Scintilla control without one, and a Chromium text field. See
//! [the focus round-trip findings](../../docs/spikes/2026-08-07-focus-round-trip.md).
//!
//! ## What is not kept
//!
//! The original text lives in the transaction and goes when it does. `commit` and `cancel` both
//! consume `self`, so there is no way to hold one open by accident, and nothing survives the
//! call that ended it.

use lexiswitch_core::layout::LayoutTable;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::IUIAutomation;

use crate::clipboard::Clipboard;
use crate::inject::{self, FocusGuard};
use crate::selection::{self, ReadVia, SelectionError};

/// How long the target is given to come back to the foreground before anything is pasted.
///
/// The round-trip spike measured a handback needing no settle at all on Notepad — 0 ms passed
/// as reliably as 300 ms — so this is margin for applications that were not measured rather
/// than a value anything is known to need. It is deliberately small: the cost of being wrong
/// in this direction is pasting into the wrong window.
const FOREGROUND_SETTLE: std::time::Duration = std::time::Duration::from_millis(60);

/// A conversion the user has started and not yet committed.
///
/// Holds the original selection and everything needed to undo the reading of it. Consumed by
/// [`Self::commit`] or [`Self::cancel`]; there is no third way out.
pub struct Transaction {
    original: String,
    via: ReadVia,
    /// The window the text was selected in, to be given the foreground back.
    target: FocusGuard,
    /// What the clipboard held before the read, when the read disturbed it.
    clipboard_before: Option<String>,
    /// Whether reading cost the clipboard something that is not text.
    lost_nontext: bool,
    /// Whether the read touched the clipboard at all. The automation path does not.
    clipboard_disturbed: bool,
    owner: HWND,
    attempts: u32,
}

impl Transaction {
    /// Read the current selection and snapshot what will be needed to put things back.
    ///
    /// Modifies nothing. Call this while the application holding the selection still has
    /// focus — that is the point of the split.
    ///
    /// The read path is chosen per invocation and never remembered. The same Chromium window
    /// was observed exposing no text pattern on one run and a working one on the next, so a
    /// cached per-application decision would be wrong about half the time.
    pub fn begin(
        automation: Option<&IUIAutomation>,
        owner: HWND,
        allow_clipboard: bool,
        attempts: u32,
    ) -> Result<Self, SelectionError> {
        let target = FocusGuard::capture();

        // Automation first: it reads without touching the clipboard, so a transaction that is
        // cancelled leaves absolutely nothing behind.
        if let Some(automation) = automation {
            if let Some(original) = selection::read_via_automation(automation) {
                return Ok(Self {
                    original,
                    via: ReadVia::Automation,
                    target,
                    clipboard_before: None,
                    lost_nontext: false,
                    clipboard_disturbed: false,
                    owner,
                    attempts,
                });
            }
        }

        if !allow_clipboard {
            return Err(SelectionError::Unsupported);
        }

        let mut read = selection::read_via_clipboard(owner, attempts)?;
        let Some(original) = read.text.take() else {
            // Nothing was copied, which the sentinel makes distinguishable from copying the
            // text that was already there. Put the clipboard back before reporting.
            let _ = read.restore(owner, attempts);
            return Err(SelectionError::NoSelection);
        };

        Ok(Self {
            original,
            via: ReadVia::Clipboard,
            target,
            clipboard_before: read.previous.take(),
            lost_nontext: read.lost_nontext,
            clipboard_disturbed: true,
            owner,
            attempts,
        })
    }

    /// The text the user selected. Every candidate is rendered from this, never from another
    /// candidate's result.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// How the selection was read, for reporting rather than for deciding anything later.
    pub fn via(&self) -> ReadVia {
        self.via
    }

    /// Whether reading cost the clipboard non-text content.
    pub fn lost_nontext(&self) -> bool {
        self.lost_nontext
    }

    /// Render the original under a pair of layouts, without touching the document.
    ///
    /// Present so a caller previewing candidates cannot accidentally render from anything but
    /// the original.
    pub fn render(&self, from: &LayoutTable, to: &LayoutTable) -> String {
        lexiswitch_core::candidates::render(&self.original, from, to)
    }

    /// Replace the selection with `replacement`, then put everything back.
    ///
    /// Restores the foreground first, because the paste has to land in the application the
    /// text came from and the chooser has had focus in the meantime.
    pub fn commit(self, replacement: &str) -> Result<Committed, SelectionError> {
        // Refuse before disturbing anything. Pasting an empty string over a selection deletes
        // it, which is the worst outcome this operation can have.
        if replacement.is_empty() {
            let _ = self.put_clipboard_back();
            return Err(SelectionError::NoChange {
                via: self.via,
                len: 0,
            });
        }

        if !self.target.restore_foreground() {
            // Without the foreground there is nowhere safe to paste: whatever is in front now
            // would receive it instead.
            let _ = self.put_clipboard_back();
            return Err(SelectionError::Inject(inject::Abandoned::FocusMoved));
        }
        std::thread::sleep(FOREGROUND_SETTLE);

        // The automation read never touched the clipboard, so there is nothing snapshotted —
        // but the *write* is a paste, and it will. Snapshot now, at the last possible moment,
        // so a transaction that is cancelled leaves the clipboard genuinely untouched and a
        // committed one still puts back what the user had.
        //
        // Getting this wrong empties the clipboard instead of restoring it, because "no
        // snapshot" and "the clipboard was empty" are the same value.
        let (before, lost_nontext) = if self.clipboard_disturbed {
            (self.clipboard_before.clone(), self.lost_nontext)
        } else {
            match Clipboard::open(self.owner, self.attempts) {
                Ok(clip) => (clip.text(), clip.holds_nontext()),
                Err(e) => return Err(SelectionError::Busy(e)),
            }
        };

        let pasted = selection::paste_over_selection(
            &self.target,
            self.owner,
            replacement,
            self.attempts,
            before,
        );

        // The paste path restores the clipboard itself; this covers the automation read, which
        // never disturbed it, and any path that failed before getting that far.
        if pasted.is_err() {
            let _ = self.put_clipboard_back();
        }
        pasted?;

        Ok(Committed {
            via: self.via,
            lost_nontext,
        })
    }

    /// Abandon the conversion, leaving the document exactly as it was.
    pub fn cancel(self) -> Result<(), SelectionError> {
        let returned = self.target.restore_foreground();
        let restored = self.put_clipboard_back();
        // Focus is reported first: a user who cancels and finds themselves typing into the
        // wrong window has a worse problem than a clipboard that was not put back.
        if !returned {
            return Err(SelectionError::Inject(inject::Abandoned::FocusMoved));
        }
        restored
    }

    /// Undo the read's effect on the clipboard, if it had one.
    fn put_clipboard_back(&self) -> Result<(), SelectionError> {
        if !self.clipboard_disturbed {
            return Ok(());
        }
        let clip = Clipboard::open(self.owner, self.attempts).map_err(SelectionError::Busy)?;
        match &self.clipboard_before {
            Some(text) => clip.set_text(text).map_err(SelectionError::Win),
            // Nothing was there before. Leaving our copy of the user's selection would be
            // worse than an empty clipboard.
            None => clip.set_text("").map_err(SelectionError::Win),
        }
    }
}

/// A conversion that was applied.
pub struct Committed {
    pub via: ReadVia,
    /// True when the clipboard held something other than text, which the round trip destroyed.
    pub lost_nontext: bool,
}
