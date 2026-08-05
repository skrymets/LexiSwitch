//! Observing keystrokes through Raw Input.
//!
//! `RIDEV_INPUTSINK` delivers keyboard input even when this window is not in the foreground,
//! and — this is the point of the whole project — it has no capability to block or alter a
//! keystroke ([ADR-0003](../../docs/adr/0003-raw-input-over-low-level-hook.md)).
//! `SetWindowsHookEx` is not called here, and nothing in this module is shaped so that
//! adding it later would be a small change.
//!
//! Known limitation, measured rather than assumed: keystrokes typed into *elevated* windows
//! never arrive. Raw Input is subject to the same UIPI restriction as a hook.

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
};

use crate::WinError;

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

/// `RI_KEY_BREAK` — set on key release.
const RI_KEY_BREAK: u16 = 0x01;

/// One observed keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Set-1 scancode, which is what the layout tables are keyed on.
    pub scancode: u16,
    pub vkey: u16,
    pub up: bool,
}

impl KeyEvent {
    /// Whether this is a modifier key rather than a character-producing one.
    ///
    /// Modifiers are excluded from the buffer for a measured reason: the spike observed a
    /// single held Shift produce fourteen indistinguishable `down` events, which would
    /// evict real typing from a 64-entry ring on their own.
    pub fn is_modifier(&self) -> bool {
        matches!(
            self.vkey,
            0x10..=0x12    // Shift, Control, Alt
                | 0x14     // Caps Lock
                | 0x5B..=0x5C // Left/Right Windows
                | 0xA0..=0xA5 // L/R Shift, Control, Alt
                | 0x90..=0x91 // Num Lock, Scroll Lock
        )
    }
}

/// Keyboard raw input registration, removed when dropped.
pub struct RawKeyboard {
    hwnd: HWND,
}

impl RawKeyboard {
    /// Register for keyboard raw input delivered to `hwnd`.
    pub fn register(hwnd: HWND) -> Result<Self, WinError> {
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            // INPUTSINK: receive input regardless of focus. Observation only — there is no
            // flag here that would let this intercept anything, which is the property the
            // security claim rests on.
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };
        // SAFETY: one fully-initialised device descriptor, with the element size the API
        // expects. hwnd is a live window owned by this thread.
        unsafe {
            RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
                .map_err(|e| WinError::new("RegisterRawInputDevices", e))?;
        }
        Ok(Self { hwnd })
    }
}

impl Drop for RawKeyboard {
    fn drop(&mut self) {
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_REMOVE,
            // RIDEV_REMOVE requires a null target; passing the window is an error.
            hwndTarget: HWND(std::ptr::null_mut()),
        };
        let _ = self.hwnd;
        // SAFETY: same shape as registration, with the documented null target for removal.
        unsafe {
            let _ = RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        }
    }
}

/// Decode a `WM_INPUT` message into a keystroke.
///
/// Returns `None` for anything that is not a keyboard event, and for the synthetic
/// `0xFF` overrun code some keyboards emit.
pub fn decode(lparam: isize) -> Option<KeyEvent> {
    let mut raw = RAWINPUT::default();
    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
    let header = std::mem::size_of::<RAWINPUTHEADER>() as u32;

    // SAFETY: `raw` is a real RAWINPUT and `size` is its true size, so GetRawInputData
    // cannot write past it. The lparam of a WM_INPUT message is the HRAWINPUT handle, which
    // is valid until the message is dispatched to DefWindowProc.
    let written = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam as *mut c_void),
            RID_INPUT,
            Some(&mut raw as *mut _ as *mut c_void),
            &mut size,
            header,
        )
    };
    if written == u32::MAX || raw.header.dwType != RIM_TYPEKEYBOARD.0 {
        return None;
    }

    // SAFETY: dwType reported keyboard, so the keyboard arm of the union is the live one.
    let kb = unsafe { raw.data.keyboard };
    if kb.VKey == 0xFF {
        // Part of an escaped sequence rather than a key in its own right.
        return None;
    }
    if kb.ExtraInformation == crate::inject::SYNTHETIC_MARKER {
        // Our own correction, coming back around. SendInput events travel the normal input
        // path, so without this the characters we just typed would be recorded as if the
        // user had typed them — refilling the buffer with the correction immediately after
        // the conversion wiped it.
        return None;
    }

    Some(KeyEvent {
        scancode: kb.MakeCode,
        vkey: kb.VKey,
        up: kb.Flags & RI_KEY_BREAK != 0,
    })
}

/// A keystroke worth recording: a physical position plus the shift state it was typed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keystroke {
    pub scancode: u16,
    pub shift: bool,
}

/// Filters raw keystrokes down to the ones worth recording, tracking shift as it goes.
///
/// Drops key releases, drops modifiers, and collapses auto-repeat — a held key produces a
/// stream of `down` events with nothing marking them as repeats, so without this a single
/// leaned-on key fills the buffer. Shift is still *observed* while being dropped, because
/// the buffer stores it per keystroke and case falls out of it.
#[derive(Debug, Default)]
pub struct KeyFilter {
    last_down: Option<u16>,
    shift_held: bool,
}

impl KeyFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shift_held(&self) -> bool {
        self.shift_held
    }

    /// Returns the keystroke if it should reach the buffer.
    pub fn accept(&mut self, event: KeyEvent) -> Option<Keystroke> {
        // Shift is tracked on both edges before anything is discarded.
        if matches!(event.vkey, 0x10 | 0xA0 | 0xA1) {
            self.shift_held = !event.up;
        }

        if event.up {
            // A release ends the repeat run for that key.
            if self.last_down == Some(event.scancode) {
                self.last_down = None;
            }
            return None;
        }
        if event.is_modifier() {
            return None;
        }
        if self.last_down == Some(event.scancode) {
            // Same key still held: auto-repeat.
            return None;
        }
        self.last_down = Some(event.scancode);
        Some(Keystroke {
            scancode: event.scancode,
            shift: self.shift_held,
        })
    }
}
