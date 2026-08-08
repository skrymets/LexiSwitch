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
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RMENU, VK_RSHIFT, VK_RWIN,
    VK_V,
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

/// How long the modifiers must read as *up* before the chord is sent.
///
/// Longer than one auto-repeat period, and that is the whole point. While a key is physically
/// held the keyboard repeats it — Windows' default rate is about thirty a second — so releasing
/// it synthetically buys roughly 33 ms before the hardware presses it again. A chord sent inside
/// that window is fine; one sent just after it arrives with the modifier down again, and the
/// release might as well not have happened.
///
/// Ninety milliseconds spans about three repeat periods, so a modifier that is genuinely still
/// held cannot look settled by luck.
const MODIFIER_STABLE: std::time::Duration = std::time::Duration::from_millis(90);

/// How often to re-check while waiting for that.
const MODIFIER_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// How long to keep trying before sending the chord anyway.
///
/// Generous, because the wait ends the moment the user lets go and the alternative to waiting
/// is being wrong. Measured against a simulated auto-repeat, an 800 ms deadline still failed
/// three times in four when the key was held for 1.4 s — the transaction gave up and sent a
/// chord it already knew would be mangled. Reaching this deadline is now reported rather than
/// papered over, so the bound exists to stop a stuck key hanging the worker, not to time-box a
/// person.
const MODIFIER_DEADLINE: std::time::Duration = std::time::Duration::from_millis(2500);

/// The modifiers a chord must not carry. `Ctrl` is absent deliberately — see below.
const HELD_MODIFIERS: [VIRTUAL_KEY; 6] =
    [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];

/// Whether one key is physically down.
fn is_down(vk: &VIRTUAL_KEY) -> bool {
    // SAFETY: no preconditions; reports whether one key is currently down. The high bit is the
    // pressed state, which is the sign bit of the i16 returned.
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

/// Release any modifier the user is still physically holding.
///
/// The hotkey is a chord — `Win+Shift+Pause` and `Win+Ctrl+Pause` by default. `RegisterHotKey`
/// consumes the `Pause`, but nothing releases the modifiers, and the user's hand is still on
/// them when the transaction starts.
/// Every chord synthesised from here then arrives at the target with that modifier added:
/// `Ctrl+C` is delivered as `Ctrl+Shift+C`.
///
/// That is not a cosmetic difference. Measured in Notepad++ against the same selection:
///
/// | sent | copied | document |
/// |---|---|---|
/// | `Ctrl+C` | the selection | intact |
/// | `Ctrl+C` with `Shift` held | nothing | **emptied** |
///
/// The copy returning nothing is reported honestly as "nothing is selected" — and by then the
/// user's text is already gone, destroyed by a keystroke this program sent. Notepad tolerates
/// the same chord, which is why this survived being probed once and dismissed.
///
/// `Ctrl` is deliberately not released: the chords here supply it themselves, and a user
/// holding it changes nothing about what they mean.
///
/// The release is not undone afterwards. The physical key is still down, so the user's own
/// release will follow and a duplicate key-up is harmless — whereas re-pressing a modifier on
/// their behalf would leave it stuck down if they let go in between.
///
/// ## Releasing once is not enough
///
/// A held key repeats. Windows types it again about thirty times a second for as long as the
/// finger is down, so a single synthetic release is undone by the hardware within about 33 ms,
/// and a chord sent after that arrives with the modifier down exactly as if nothing had been
/// released. This is why the *order the user pressed the modifiers in* changed the outcome:
/// typematic repeat applies to the last key pressed, so `Win` then `Alt` leaves **Alt**
/// repeating, while `Alt` then `Win` leaves `Win` repeating — and only one of those two ruins
/// the chord.
///
/// It ruins it because `Alt` does not merely add a modifier: a keystroke with `Alt` down is
/// delivered as `WM_SYSKEYDOWN` rather than `WM_KEYDOWN`, a different message that an editor's
/// key handling does not answer at all. `Ctrl+C` under a repeating `Alt` is not a weaker copy,
/// it is not a copy. `Win` never changes the message, which is why `Win+Shift+Pause` and
/// `Shift+Win+Pause` both work in either order while `Alt+Win+Pause` does not.
///
/// So this releases and then *waits for the release to hold*, re-releasing anything the keyboard
/// presses again, until every modifier has read as up for longer than a repeat period. A user
/// who simply keeps holding the key is not waited for indefinitely — see [`MODIFIER_DEADLINE`].
///
/// Returns whether the modifiers actually settled. Callers proceed either way; the answer is
/// worth having only because a chord sent without settling is the one that gets mangled.
fn release_held_modifiers() -> bool {
    // Nothing held means nothing to wait for. Worth checking first: the tray command and a
    // shortcut the user has already let go of are the common cases, and neither should pay for
    // a stability window it does not need.
    if !HELD_MODIFIERS.iter().any(is_down) {
        return true;
    }

    let start = std::time::Instant::now();
    let mut up_since: Option<std::time::Instant> = None;
    while start.elapsed() < MODIFIER_DEADLINE {
        let ups: Vec<INPUT> = HELD_MODIFIERS
            .into_iter()
            .filter(is_down)
            .map(|vk| key(vk, true))
            .collect();

        if ups.is_empty() {
            let since = *up_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= MODIFIER_STABLE {
                return true;
            }
        } else {
            // Down again — either never released, or the keyboard repeated it. Either way the
            // stability window starts over.
            up_since = None;
            // SAFETY: a live slice of fully-initialised INPUT structures with the element size
            // the API requires. A short count means some releases were not delivered, which the
            // next pass through this loop will notice and retry.
            unsafe {
                SendInput(&ups, std::mem::size_of::<INPUT>() as i32);
            }
        }
        std::thread::sleep(MODIFIER_POLL);
    }
    false
}

/// Send Ctrl+<vk> as a press-and-release pair around the modifier.
fn chord(vk: VIRTUAL_KEY) -> Result<(), Abandoned> {
    // Before anything else: a modifier the user has not let go of is part of every chord sent
    // from here, and in at least one editor that turns a copy into a deletion — measured, any
    // of Alt, Win or Shift stops Ctrl+C copying in Notepad++ entirely.
    //
    // Refusing beats guessing. Sending the chord anyway produces "nothing is selected", which
    // is a statement about the document and is false; saying a key is held is a statement about
    // the keyboard and is true, and the user can act on it without wondering what went wrong.
    if !release_held_modifiers() {
        return Err(Abandoned::ModifiersHeld);
    }

    // One batch, not two. Waiting for the modifiers to read as up is necessary but not
    // sufficient: `GetAsyncKeyState` answers about the state *now*, while the target reads its
    // own input queue in order, and an auto-repeat event can be queued between a release sent in
    // one call and a chord sent in the next. `SendInput` delivers a single call's events as one
    // uninterruptible block, so putting the last releases and the chord in the same array closes
    // that window instead of narrowing it.
    let mut inputs: Vec<INPUT> = HELD_MODIFIERS
        .into_iter()
        .filter(is_down)
        .map(|vk| key(vk, true))
        .collect();
    inputs.extend([
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ]);
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
    /// A modifier the user is holding could not be released, so no chord could be sent that
    /// they would recognise. Distinct from every other failure because it is the only one the
    /// user can clear instantly, by taking their hand off the key.
    ModifiersHeld,
    Win(WinError),
}

impl std::fmt::Display for Abandoned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FocusMoved => write!(
                f,
                "focus moved before the correction could be applied; nothing was changed"
            ),
            Self::ModifiersHeld => write!(
                f,
                "a key from the shortcut is still held down, and it would have altered what was \n                 sent; let go and press it again"
            ),
            Self::Win(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Abandoned {}
