//! Activating a keyboard layout for the foreground window.
//!
//! After a correction, switching to the layout the text was converted *into* means the next
//! thing typed continues in that language rather than repeating the mistake. Configurable,
//! because it is a guess about intent — see `switch_layout_after_correction`.

use std::ffi::c_void;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

use crate::WinError;

/// Ask the foreground window to switch to `hkl`.
///
/// Posted rather than sent: the target processes it on its own thread when ready, so a busy
/// or unresponsive application cannot stall our message loop — which must never block, since
/// it is also what keeps the tray alive.
///
/// The request is advisory. An application may decline, and there is no reliable way to
/// learn that it did, so the caller should not treat success here as proof the layout
/// changed.
///
/// `hkl_raw` must be the handle at its full width. Truncating it to 32 bits loses the
/// sign extension on handles whose high word is a device identifier, and the request is then
/// made for a layout that does not exist — which fails exactly as being declined does, with
/// no error anywhere.
pub fn activate_for_foreground(hkl_raw: usize) -> Result<(), WinError> {
    // SAFETY: a null foreground window is possible and PostMessageW rejects it, which is
    // reported rather than ignored. The HKL is reconstructed from a value obtained from
    // GetKeyboardLayoutList, so it names a layout this session knows about; handles are
    // opaque integers and round-tripping the bit pattern is how they are stored.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return Err(WinError::last("GetForegroundWindow"));
        }
        let hkl = HKL(hkl_raw as *mut c_void);
        PostMessageW(
            Some(hwnd),
            WM_INPUTLANGCHANGEREQUEST,
            // 0 rather than INPUTLANGCHANGE_SYSCHARSET: the layout is named explicitly in
            // lParam, so no charset-based selection is wanted.
            WPARAM(0),
            LPARAM(hkl.0 as isize),
        )
        .map_err(|e| WinError::new("PostMessageW(WM_INPUTLANGCHANGEREQUEST)", e))
    }
}
