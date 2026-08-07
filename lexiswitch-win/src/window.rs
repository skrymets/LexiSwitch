//! The hidden message-only window and its message loop.
//!
//! Everything Windows delivers asynchronously — hotkeys, tray clicks, worker wake-ups —
//! arrives as a message to this window. It paints nothing and is never shown.
//!
//! **It is deliberately not message-only.** `HWND_MESSAGE` would be the natural choice for a
//! window that only receives messages, and it was the original one, but a message-only window
//! can never become the foreground window. The tray menu needs exactly that: `TrackPopupMenu`
//! must be anchored to a foreground window or it has no owner to dismiss against, so the menu
//! either never appeared or vanished immediately. It is instead an ordinary top-level window
//! that is never given `WS_VISIBLE`, with `WS_EX_TOOLWINDOW` to keep it out of the taskbar and
//! Alt-Tab in case it ever is shown.
//!
//! The loop must never block. UI Automation and clipboard calls can stall for seconds, and
//! this same loop is what keeps the tray responsive, so slow work belongs on the worker
//! thread with only a snapshot sent across.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, InSendMessage,
    PostMessageW, PostQuitMessage, RegisterClassW, MSG, WM_APP, WM_DESTROY, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchMessageWindow");

/// Posted by [`Waker::wake`] to break the loop out of `GetMessageW`.
///
/// The loop blocks until a message arrives, so work finished on another thread is not noticed
/// until something happens to wake it. While Raw Input was registered every keystroke anywhere
/// delivered `WM_INPUT` and pumped the loop continuously, which hid the need for this; with
/// observation removed, an idle loop stays idle and a result produced meanwhile is never read.
pub const WM_WAKE: u32 = WM_APP + 2;

/// Permission to wake a [`MessageWindow`] from another thread.
///
/// Holds the window as an integer because a raw `HWND` is not `Send`. It is only ever posted
/// to, never dereferenced, which is what makes moving it between threads sound.
#[derive(Debug, Clone, Copy)]
pub struct Waker {
    hwnd: isize,
}

// SAFETY: the value is an opaque window identifier this type never dereferences — it posts to
// it and nothing else. Moving that between threads cannot create a data race, and posting to a
// window that has since been destroyed fails harmlessly.
unsafe impl Send for Waker {}

impl Waker {
    /// Wake the loop so it looks at whatever the caller has just made ready.
    pub fn wake(&self) {
        // SAFETY: posting an application-defined message to an opaque window identifier. A
        // stale handle makes PostMessageW return false, which is the whole error handling
        // this needs — there is nothing to clean up either way.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.hwnd as *mut std::ffi::c_void)),
                WM_WAKE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

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
    ///
    /// The window is created but never shown, so it has no visible presence despite being a
    /// top-level window. See the module note for why it is not message-only.
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
                // Out of the taskbar and Alt-Tab. Belt and braces: without WS_VISIBLE the
                // window is never shown at all.
                WS_EX_TOOLWINDOW,
                CLASS_NAME,
                w!("LexiSwitch"),
                // Top level, and no WS_VISIBLE. A parent of HWND_MESSAGE here would make the
                // window unable to take the foreground, which the tray menu requires.
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
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

    /// A handle another thread can use to wake this loop.
    pub fn waker(&self) -> Waker {
        Waker {
            hwnd: self.hwnd.0 as isize,
        }
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

    // Turn a *sent* application message into a *posted* one.
    //
    // `GetMessageW` returns posted messages only. A message another thread sends is not queued
    // at all — it is dispatched straight here, inside the `GetMessageW` call, and the loop
    // never sees it. The shell sends notify-icon callbacks, so every tray click reached this
    // function and stopped, while [`MessageWindow::run`]'s handler — where this program does
    // its work — waited for something that was never going to arrive.
    //
    // Measured rather than reasoned: sending `WM_APP + 1` to this window produced nothing and
    // posting the identical message produced a callback, which is also why hand-posting opened
    // the tray menu while a real right-click did not.
    //
    // Restricted to the application range so system messages, which are sent constantly and
    // whose return values matter, keep going to the default handler. The re-posted copy comes
    // back through the queue with `InSendMessage` false, so it is dispatched once and not
    // bounced again.
    //
    // SAFETY: `InSendMessage` has no preconditions, and posting to the window this procedure
    // was called for cannot outlive it — a failed post is reported as a bool this ignores.
    if msg >= WM_APP && unsafe { InSendMessage() }.as_bool() {
        // SAFETY: posting to the window this procedure was invoked for, which cannot have been
        // destroyed while it is running. A failed post returns false, which is ignored because
        // there is nothing to clean up and nowhere better to report it from.
        unsafe {
            let _ = PostMessageW(Some(hwnd), msg, wp, lp);
        }
        return LRESULT(0);
    }

    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}
