//! Synthesising the few chords a conversion needs.
//!
//! Replacement is a paste ([ADR-0010](../../docs/adr/0010-paste-selection-replacement.md)):
//! the converted text goes on the clipboard and `Ctrl+V` delivers it as one logical edit. So
//! this module sends four events per operation regardless of how much text is involved, and
//! nothing here scales with length.
//!
//! It used to type replacements out one character at a time. That cost one undo step per
//! character, and `SendInput` reports how many events it enqueued rather than how many the
//! target applied — so a write an application mangled reported success. Both defects and the
//! measurements behind them are recorded in
//! [the delivery findings](../../docs/spikes/2026-08-06-selection-replacement-delivery.md).
//!
//! **Focus can move between deciding and acting.** A chord delivered to the wrong window acts
//! on someone else's text, so the foreground window is checked immediately before sending and
//! the operation is abandoned if it moved. This cannot be closed entirely with synthetic
//! input, which is a further reason to prefer one paste over a long sequence.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C,
    VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use crate::WinError;

/// The foreground window at a point in time, for detecting that focus moved.
///
/// Stored as an integer rather than an `HWND` so it can cross a thread boundary. A window
/// handle is a process-wide identifier, not thread-affine, for the comparisons made here —
/// it is only Rust's raw-pointer representation that makes `HWND` non-`Send`. The guard is
/// captured on the message-loop thread and checked on the worker, which is the whole point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusGuard {
    hwnd: isize,
}

// SAFETY: the value is an opaque window identifier that this type never dereferences — it
// only compares it and hands it back to the window manager. Moving that between threads
// cannot create a data race.
unsafe impl Send for FocusGuard {}

impl FocusGuard {
    /// Capture the current foreground window.
    pub fn capture() -> Self {
        // SAFETY: no preconditions. A null handle when nothing is focused is a valid
        // answer, and compares unequal to any real window later.
        let hwnd = unsafe { GetForegroundWindow() };
        Self {
            hwnd: hwnd.0 as isize,
        }
    }

    /// Whether the same window is still in front.
    pub fn unchanged(&self) -> bool {
        // SAFETY: as above.
        let now = unsafe { GetForegroundWindow() };
        !now.is_invalid() && now.0 as isize == self.hwnd
    }

    /// The captured window.
    pub fn window(&self) -> HWND {
        HWND(self.hwnd as *mut std::ffi::c_void)
    }

    /// Bring the captured window back to the foreground.
    ///
    /// Needed because a chooser has to take focus to read the keyboard, so the application
    /// being edited loses it and must have it back before anything is pasted. Windows permits
    /// this in the case that matters here — a process giving the foreground away from a window
    /// it already owns — and the round trip was measured before the design depended on it, in
    /// [the focus round-trip findings](../../docs/spikes/2026-08-07-focus-round-trip.md):
    /// the selection survives, and the handback needed no settle at all.
    ///
    /// Returns whether Windows accepted it. A refusal is worth reporting rather than pasting
    /// into whatever happens to be in front instead.
    pub fn restore_foreground(&self) -> bool {
        // SAFETY: no preconditions. Passing a stale handle makes the call fail, which is the
        // bool this returns.
        unsafe { SetForegroundWindow(self.window()) }.as_bool()
    }
}

/// Send Ctrl+C, to make the target copy its selection.
pub fn copy_selection() -> Result<(), Abandoned> {
    chord(VK_C)
}

/// Send Ctrl+V, to make the target paste over its selection.
///
/// Refuses to act if focus has moved since `guard` was captured — pasting into the wrong
/// window destroys whatever was selected there.
pub fn paste(guard: &FocusGuard) -> Result<(), Abandoned> {
    if !guard.unchanged() {
        return Err(Abandoned::FocusMoved);
    }
    chord(VK_V)
}

/// Send Ctrl+<vk> as a press-and-release pair around the modifier.
fn chord(vk: VIRTUAL_KEY) -> Result<(), Abandoned> {
    let inputs = [
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ];
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the
    // API requires.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(Abandoned::Win(WinError::last("SendInput")));
    }
    Ok(())
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                // Injected input used to carry a marker so this program's own Raw Input
                // registration could recognise and discard it. Nothing observes the keyboard
                // any more, so nothing reads it.
                dwExtraInfo: 0,
            },
        },
    }
}

/// Why an operation did not happen.
#[derive(Debug)]
pub enum Abandoned {
    /// Focus moved between deciding to act and sending. Nothing was delivered.
    FocusMoved,
    Win(WinError),
}

impl std::fmt::Display for Abandoned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FocusMoved => write!(
                f,
                "focus moved before the correction could be applied; nothing was changed"
            ),
            Self::Win(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Abandoned {}
