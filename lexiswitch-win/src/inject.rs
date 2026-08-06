//! Replacing text by synthesising input.
//!
//! Correction works retroactively: backspace over what is there, type what was meant. No
//! keystroke is ever suppressed before the focused application sees it, which is why
//! observation alone is enough and no hook is needed
//! ([ADR-0003](../../docs/adr/0003-raw-input-over-low-level-hook.md)).
//!
//! Two hazards are handled here.
//!
//! **Synthetic input is observable, including by us.** `SendInput` events travel the normal
//! input path, so our own Raw Input registration sees every character we inject. Left alone,
//! correcting a word would refill the buffer with the correction. Every injected event
//! therefore carries [`SYNTHETIC_MARKER`] in its extra information, and
//! [`crate::rawinput::decode`] drops anything bearing it.
//!
//! **Focus can move mid-sequence.** Backspaces delivered to the wrong window delete someone
//! else's text. The foreground window is checked immediately before injecting and the
//! operation is abandoned if it moved. This cannot be eliminated entirely with synthetic
//! input — a change during the sequence is still possible — so sequences are kept short.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK, VK_C, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::WinError;

/// Tags input this process synthesised, so it can be told apart from real typing.
///
/// Arbitrary but distinctive. Windows passes `dwExtraInfo` through to the `ExtraInformation`
/// field of the raw keyboard event untouched, which is what makes the round trip work.
pub const SYNTHETIC_MARKER: u32 = 0x4C58_0001; // "LX"

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
}

/// Delete `backspaces` characters and type `text` in their place.
///
/// Refuses to do anything if focus has moved since `guard` was captured — a partial
/// correction in the wrong window is worse than no correction at all.
///
/// The count is in characters, not UTF-16 units or bytes: it must match what the target
/// application will delete per backspace, and it comes from the buffer's own record rather
/// than being inferred from the document.
pub fn replace(guard: &FocusGuard, backspaces: usize, text: &str) -> Result<(), Abandoned> {
    if !guard.unchanged() {
        return Err(Abandoned::FocusMoved);
    }

    let mut inputs: Vec<INPUT> = Vec::with_capacity(backspaces * 2 + text.len() * 2);
    for _ in 0..backspaces {
        inputs.push(key(VK_BACK, false));
        inputs.push(key(VK_BACK, true));
    }
    for unit in text.encode_utf16() {
        // Surrogate pairs go through as two units; each is sent as its own event, which is
        // what KEYEVENTF_UNICODE expects.
        inputs.push(unicode(unit, false));
        inputs.push(unicode(unit, true));
    }

    if inputs.is_empty() {
        return Ok(());
    }

    // Re-check as late as possible. There is still a window between this and the call, and
    // it cannot be closed with synthetic input — hence short sequences.
    if !guard.unchanged() {
        return Err(Abandoned::FocusMoved);
    }

    // SAFETY: `inputs` is a live slice of fully-initialised INPUT structures, and the size
    // passed is the element size the API requires.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(Abandoned::Win(WinError::last("SendInput")));
    }
    Ok(())
}

/// Send Ctrl+C, to make the target copy its selection.
///
/// Carries the same synthetic marker as everything else this module sends, so our own Raw
/// Input registration ignores it.
pub fn copy_selection() -> Result<(), Abandoned> {
    chord(VK_C)
}

/// Send Ctrl+V, to make the target paste over its selection.
///
/// This is how a selection is replaced ([ADR-0010](../../docs/adr/0010-paste-selection-replacement.md)):
/// four events regardless of how much text is being delivered, applied by the target as a
/// single edit. [`replace`] remains for last-word correction, which needs backspaces that a
/// paste cannot supply.
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
                dwExtraInfo: SYNTHETIC_MARKER as usize,
            },
        },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                // KEYEVENTF_UNICODE carries the character in wScan with no virtual key, so
                // the target receives the character regardless of its active layout.
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: if up {
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                } else {
                    KEYEVENTF_UNICODE
                },
                time: 0,
                dwExtraInfo: SYNTHETIC_MARKER as usize,
            },
        },
    }
}

/// Why a replacement did not happen.
#[derive(Debug)]
pub enum Abandoned {
    /// Focus moved between deciding to correct and injecting. Nothing was typed.
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
