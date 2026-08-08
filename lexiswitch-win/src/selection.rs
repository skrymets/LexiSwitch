//! Converting the current selection.
//!
//! Automation first, clipboard second ([ADR-0006](../../docs/adr/0006-uia-first-clipboard-fallback.md)).
//! Detection is by *attempt*: try the automation path, fall back on failure. There is no
//! list of known-good applications, because such a list is permanently out of date and fails
//! in exactly the direction that surprises the user.
//!
//! ## What each path actually does
//!
//! The two paths differ in how the selection is **read**. They are now identical in how it is
//! written ([ADR-0010](../../docs/adr/0010-paste-selection-replacement.md)).
//!
//! - **Automation** reads the selected text through UI Automation's text pattern, touching
//!   nothing else.
//! - **Clipboard fallback** snapshots the clipboard, sends Ctrl+C, reads what landed there.
//!
//! Both then replace the selection by putting the converted text on the clipboard and sending
//! Ctrl+V, and both restore what was on the clipboard afterwards. UI Automation's text pattern
//! is read-only — it can tell you what is selected but not change it — so "automation
//! replacement" is not a thing that exists, and no amount of wanting makes it one.
//!
//! **The automation path no longer avoids the clipboard.** It avoids it for the *read*, which
//! is why it is still tried first, but the write takes the clipboard on every conversion. The
//! payload is written with the Clipboard History opt-out formats set; that is a weaker
//! guarantee than never touching it, and ADR-0010 is where that trade is argued.
//!
//! Replacement used to be per-character keystroke synthesis. It cost one undo step per
//! character and could be silently mangled by an application that consumed input
//! asynchronously — `SendInput` reports what it enqueued, not what arrived. Both are recorded
//! in [the delivery findings](../../docs/spikes/2026-08-06-selection-replacement-delivery.md).

use lexiswitch_core::layout::LayoutTable;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};

use crate::clipboard::{self, Clipboard};
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

/// Planted before the copy, so that a copy which does nothing is distinguishable from one that
/// produced the text already on the clipboard.
const COPY_SENTINEL: &str = "\u{1}lexiswitch-copy-sentinel";

/// How long the target is given to service the `Ctrl+C` before the copy is called a failure.
///
/// This used to be a flat 60 ms sleep, and that was a race the product lost silently. A copy
/// that had not landed yet was indistinguishable from no selection at all, so the user was told
/// "nothing is selected" about text they could see was selected.
///
/// Measured on this machine, Notepad++ services the copy in about 32 ms with a worst case of 44
/// — inside 60, but by less than a factor of two, and a heavier application or a busy machine
/// crosses it. The paste path had already learned this lesson and waits properly
/// ([`clipboard::wait_for_consumer`]); the read path had not.
///
/// Generous because the cost of being wrong is asymmetric. Waiting too long delays a
/// transaction the user is watching; giving up too early reports a falsehood about their
/// document. Nothing waits the whole deadline in the normal case — the poll returns as soon as
/// the clipboard changes, which is sooner than the old fixed sleep returned.
const COPY_DEADLINE: std::time::Duration = std::time::Duration::from_millis(600);

/// How often to look while waiting. Cheap: a counter read, no clipboard handle.
const COPY_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Wait for the clipboard to change from what it held when `before` was taken.
///
/// Returns whether it changed within the deadline. False means the target never wrote anything,
/// which is what an empty selection looks like — and now also the only thing it looks like,
/// since a slow copy is no longer mistaken for one.
///
/// Polls [`clipboard::sequence_number`] rather than the contents, because reading the contents
/// means opening the clipboard, and holding it open is exactly what stops the target writing to
/// it. Waiting for something by preventing it is a good way to wait forever.
fn wait_for_copy(before: u32) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < COPY_DEADLINE {
        if clipboard::sequence_number() != before {
            return true;
        }
        std::thread::sleep(COPY_POLL);
    }
    false
}

/// Read the current selection by copying it, restoring the previous clipboard afterwards.
///
/// The previous **text** is restored; any other format is lost. On failure to acquire the
/// clipboard nothing is copied, nothing is pasted, and the clipboard is left alone.
pub fn read_via_clipboard(owner: HWND, attempts: u32) -> Result<ClipboardRead, SelectionError> {
    // Snapshot first: if this fails there is no point disturbing anything.
    let (snapshot, lost_nontext) = {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        (clip.text(), clip.holds_nontext())
    };

    // Plant a sentinel so a copy that does nothing is distinguishable from one that produced
    // the text already there. Copying an empty selection leaves the clipboard untouched, and
    // its old contents then read as though they were the selection — which is how converting a
    // Cyrillic selection reported "would not change the selection" while the program was
    // holding English text from an earlier copy.
    {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        clip.set_text(COPY_SENTINEL).map_err(SelectionError::Win)?;
    }

    // The counter as it stands with the sentinel on the clipboard. Anything the target writes
    // moves it, which is how the wait below knows the copy landed without having to look.
    let before = clipboard::sequence_number();

    // Ctrl+C, then wait for the target to act on it.
    inject::copy_selection().map_err(SelectionError::Inject)?;
    let landed = wait_for_copy(before);

    let copied = {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        clip.text()
    };
    // The sentinel survived, so nothing was copied — either the deadline passed or the target
    // wrote something that is not text.
    let copied = match copied {
        Some(text) if text == COPY_SENTINEL => None,
        _ if !landed => None,
        other => other,
    };

    Ok(ClipboardRead {
        text: copied,
        previous: snapshot,
        lost_nontext,
    })
}

/// How long the target is given to read the clipboard after `Ctrl+V` before it is restored.
///
/// Load-bearing. Restoring too early races the paste: the target reads whatever is on the
/// clipboard when it gets around to handling the message, and if the previous contents are
/// already back, that is what it pastes.
const PASTE_SETTLE: std::time::Duration = std::time::Duration::from_millis(180);

/// How long to wait for the target to take the clipboard before restoring it anyway.
///
/// Generous, because losing this race is silent and destroys the conversion: the target reads
/// the clipboard after the restore and pastes the *previous* contents into the user's document.
/// Nothing waits the whole time in practice — the wait ends as soon as the consumer lets go —
/// so the cost of a large value is paid only by an application that never reads it at all.
const PASTE_CONSUMED: std::time::Duration = std::time::Duration::from_millis(2000);

/// Deliver `text` by pasting it over the selection, then put the clipboard back.
///
/// One edit in the target and four synthetic events regardless of length, which is the whole
/// point of [ADR-0010](../../docs/adr/0010-paste-selection-replacement.md).
///
/// `previous` is what the clipboard held beforehand — captured by the caller, because the two
/// read paths learn it at different moments and neither should snapshot our own payload.
/// Replace the selection with `text`, reporting whether the target was seen to take it.
///
/// The bool is **not** proof the document changed, and it is deliberately not used to report
/// failure. It says only that some process opened the clipboard while this was waiting, which
/// is a precondition for pasting and nothing more:
///
/// - a target can open the clipboard and then decline the edit — Chrome does exactly that for
///   a read-only field, so `true` does not mean the text changed;
/// - a target can read faster than this can start watching, so `false` does not reliably mean
///   the text did not change.
///
/// It was briefly used to report "conversion was not applied", which turned a working Notepad
/// conversion into a reported failure. Detecting a refused paste needs a verification read of
/// the target, not this (`docs/manual-tests.md`).
pub fn paste_over_selection(
    guard: &FocusGuard,
    owner: HWND,
    text: &str,
    attempts: u32,
    previous: Option<String>,
) -> Result<bool, SelectionError> {
    // Never paste nothing. Pasting an empty clipboard over a selection deletes it — the worst
    // outcome a conversion can have, and indistinguishable from the program eating the user's
    // text on purpose. `convert` cannot produce this from non-empty input, so reaching here
    // means something upstream is wrong and the safe move is to do nothing at all.
    if text.is_empty() {
        return Err(SelectionError::NoChange {
            via: ReadVia::Clipboard,
            len: 0,
        });
    }

    {
        let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
        // Excluded, not plain: this is text derived from what the user typed, and Clipboard
        // History persists what it is shown. A failure here leaves the clipboard empty rather
        // than holding the payload unprotected.
        clip.set_text_excluded(text).map_err(SelectionError::Win)?;
    }

    let pasted = inject::paste(guard).map_err(SelectionError::Inject);

    // Settle first, then wait for the target to take the clipboard.
    //
    // The order is load-bearing and was established by measurement in both directions.
    //
    // Waiting *before* the settle catches the wrong open. Chrome watches the clipboard for
    // changes, so it opens it the moment anything is written — including this write. The wait
    // then returns on that observation, the restore below runs, and the real paste reads the
    // restored contents: the field ends up holding the user's previous clipboard instead of
    // the conversion. Measured, both times it was tried.
    //
    // Settling first skips past that observation so the wait sees the paste's own read. The
    // cost is that a target reading inside the settle is missed entirely, which is why
    // `consumed` is reported for diagnostics and never used to decide anything.
    std::thread::sleep(PASTE_SETTLE);
    let consumed = clipboard::wait_for_consumer(PASTE_CONSUMED);

    // Restore regardless of whether the paste worked — the clipboard must not keep our copy
    // of the user's text either way.
    let restored = ClipboardRead {
        text: None,
        previous,
        lost_nontext: false,
    }
    .restore(owner, attempts);

    pasted?;
    restored?;
    Ok(consumed)
}

/// The result of a clipboard read, carrying what must be put back.
pub struct ClipboardRead {
    pub text: Option<String>,
    pub previous: Option<String>,
    /// Whether the clipboard held something other than text when it was snapshotted, and so
    /// lost it. Recorded at snapshot time because by restore time the evidence is gone.
    pub lost_nontext: bool,
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

    // Automation first — it *reads* without touching the clipboard. The write below does
    // touch it, on this path as much as the other (ADR-0010).
    if let Some(automation) = automation {
        if let Some(text) = read_via_automation(automation) {
            let converted = lexiswitch_core::convert::convert(&text, from, to).ok_or(
                SelectionError::NoChange {
                    via: ReadVia::Automation,
                    len: text.chars().count(),
                },
            )?;

            // Snapshot only now: the read needed nothing, and taking the clipboard before
            // knowing there is a conversion to deliver would disturb it for nothing.
            let (previous, lost_nontext) = {
                let clip = Clipboard::open(owner, attempts).map_err(SelectionError::Busy)?;
                (clip.text(), clip.holds_nontext())
            };

            paste_over_selection(&guard, owner, &converted, attempts, previous)?;
            return Ok(Converted {
                via: ReadVia::Automation,
                text: converted,
                lost_nontext,
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

    let Some(converted) = lexiswitch_core::convert::convert(&text, from, to) else {
        // Put the clipboard back before reporting. The copy above overwrote it, and leaving the
        // user's clipboard holding their own selection because a conversion declined to act is
        // a side effect they never asked for.
        let _ = read.restore(owner, attempts);
        return Err(SelectionError::NoChange {
            via: ReadVia::Clipboard,
            len: text.chars().count(),
        });
    };

    paste_over_selection(&guard, owner, &converted, attempts, read.previous.take())?;

    Ok(Converted {
        via: ReadVia::Clipboard,
        text: converted,
        lost_nontext: read.lost_nontext,
    })
}

/// A successful selection conversion.
pub struct Converted {
    pub via: ReadVia,
    pub text: String,
    /// True when the clipboard held something other than text, which the round trip destroyed.
    ///
    /// Every conversion now uses the clipboard, so "was the clipboard involved" stopped being
    /// worth telling anyone. What is still worth telling them is that something on it is gone,
    /// and that is rare.
    pub lost_nontext: bool,
}

/// Why a selection was not converted. Every variant names what did *not* happen.
#[derive(Debug)]
pub enum SelectionError {
    /// No text pattern, and the clipboard fallback is disabled by configuration.
    Unsupported,
    /// Nothing appears to be selected.
    NoSelection,
    /// Conversion would not change the text. Carries what was read, because the useful
    /// question when this appears is *what* the program got hold of.
    NoChange { via: ReadVia, len: usize },
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
            Self::NoChange { via, len } => write!(
                f,
                "converting would not change the {len} characters read via {}",
                match via {
                    ReadVia::Automation => "the accessibility text pattern",
                    ReadVia::Clipboard => "copy",
                }
            ),
            Self::Busy(e) => write!(f, "{e}"),
            Self::Inject(e) => write!(f, "{e}"),
            Self::Win(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SelectionError {}
