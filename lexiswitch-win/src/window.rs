//! The hidden message-only window and its message loop.
//!
//! Everything Windows delivers asynchronously — raw input, hotkeys, tray clicks — arrives
//! as a message to this window. It is message-only (`HWND_MESSAGE`), so it never appears on
//! screen, in the taskbar, or in Alt-Tab, and it paints nothing.
//!
//! The loop must never block. UI Automation and clipboard calls can stall for seconds, and
//! this same loop is what keeps the tray responsive, so slow work belongs on the worker
//! thread with only a snapshot sent across.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WNDCLASSW,
};

use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchMessageWindow");

/// A message-only window, destroyed when dropped.
pub struct MessageWindow {
    hwnd: HWND,
}

impl MessageWindow {
    /// Register the window class if needed and create the window.
    ///
    /// Registering the same class twice fails with `ERROR_CLASS_ALREADY_EXISTS`, which is
    /// harmless — the class from the first call is still usable — so it is not treated as an
    /// error here.
    pub fn new() -> Result<Self, WinError> {
        // SAFETY: the class name is a static wide string with a NUL, and the WNDCLASSW is
        // fully initialised before registration. GetModuleHandleW(None) returns this
        // module's handle, which outlives the window.
        let hwnd = unsafe {
            let hinstance =
                GetModuleHandleW(None).map_err(|e| WinError::new("GetModuleHandleW", e))?;

            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            // A zero return can mean "already registered", which is fine; CreateWindowExW
            // below is the real check.
            let _ = RegisterClassW(&class);

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                w!("LexiSwitch"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| WinError::new("CreateWindowExW", e))?
        };

        Ok(Self { hwnd })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Pump messages until `WM_QUIT`, handing each one to `handler` before dispatching.
    ///
    /// The handler sees every message; returning `Stop` ends the loop. Messages are still
    /// dispatched afterwards because `DefWindowProcW` performs cleanup that `WM_INPUT`
    /// requires.
    pub fn run<F>(&self, mut handler: F) -> Result<(), WinError>
    where
        F: FnMut(&MSG) -> Flow,
    {
        let mut msg = MSG::default();
        loop {
            // SAFETY: msg is a valid MSG for the whole call. GetMessageW returns 0 for
            // WM_QUIT and -1 for an error, and both end the loop.
            let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if got.0 == 0 {
                return Ok(());
            }
            if got.0 == -1 {
                return Err(WinError::last("GetMessageW"));
            }

            let flow = handler(&msg);

            // SAFETY: msg was filled by GetMessageW and is untouched since.
            unsafe {
                let _ = DispatchMessageW(&msg);
            }

            if flow == Flow::Stop {
                return Ok(());
            }
        }
    }
}

/// Whether the message loop should keep going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: hwnd was created by this type and has not been destroyed elsewhere.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// `WNDPROC` requires `extern "system"`, and windows-rs exposes `DefWindowProcW` as a plain
/// Rust `unsafe fn`, so it cannot be handed over directly. Established by the spike.
///
/// SAFETY (contract on the caller, which is Windows): invoked only by the window manager
/// for a window of this class, with a valid `hwnd` and with `wp`/`lp` matching whatever
/// `msg` defines them to be. Nothing here dereferences either, so the only obligation this
/// function itself carries is forwarding them unchanged to the default handler.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        // SAFETY: no preconditions; posts WM_QUIT to this thread's queue.
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}
