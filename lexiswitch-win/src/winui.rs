//! The few window mechanics the chooser and the recorder both need.
//!
//! Two small windows now exist, built the same way for the same reasons: they take focus
//! because [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md) leaves no other way to
//! read a keyboard, and they draw with raw GDI because the 5 MB budget rules out a toolkit.
//! Everything here was written for the chooser and is shared rather than copied — the
//! foreground handling in particular is subtle enough that two versions would drift.

use windows::core::w;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, GetDC, GetDeviceCaps, GetMonitorInfoW, GetTextMetricsW, MonitorFromWindow,
    ReleaseDC, SelectObject, CLEARTYPE_QUALITY, DEFAULT_CHARSET, FF_DONTCARE,
    FONT_CLIP_PRECISION, FW_NORMAL, FW_SEMIBOLD, HFONT, HGDIOBJ, LOGPIXELSY, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, OUT_TT_PRECIS, TEXTMETRICW,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
};

/// Pixels per logical unit at the primary display's DPI, as a whole-number scale.
///
/// Read from a screen device context rather than `GetDpiForWindow`, so no window is needed and
/// no further windows-rs feature is pulled in for it.
pub fn dpi_scale() -> i32 {
    // SAFETY: a null HDC asks GetDeviceCaps about the primary display, which is what is wanted
    // before any window exists.
    let dpi = unsafe { GetDeviceCaps(None, LOGPIXELSY) };
    if dpi <= 0 {
        1
    } else {
        (dpi / 96).max(1)
    }
}

/// A UI font at the display's scale. The caller owns it and must delete it.
pub fn ui_font(scale: i32, emphasised: bool) -> HFONT {
    let height = -(11 * 96 * scale / 72);
    // SAFETY: every argument is a plain value and the face name is a static NUL-terminated wide
    // string. A failure returns a null HFONT, which SelectObject tolerates by leaving the DC's
    // current font in place.
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            if emphasised { FW_SEMIBOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_TT_PRECIS,
            FONT_CLIP_PRECISION(0),
            CLEARTYPE_QUALITY,
            FF_DONTCARE.0 as u32,
            // Segoe UI covers Cyrillic, which is most of what these windows render.
            w!("Segoe UI"),
        )
    }
}

/// The height of one row and the width of one character, for a font, before any window exists.
///
/// Measured rather than assumed because the layout advances by these numbers and the window is
/// sized from them; a guess in one place and a measurement in the other is what clipped the
/// chooser's last line.
///
/// Returns `(row height, wrapped-line height, character width)`. The middle value is what
/// `DrawTextW` uses when it wraps, which is *not* the row height — the row has padding and the
/// wrapped line does not, and confusing them puts a clip boundary through the middle of a line.
pub fn metrics(font: HFONT, scale: i32) -> (i32, i32, i32) {
    // SAFETY: a null window handle asks for a screen DC, which needs no window to exist.
    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return (22 * scale, 16 * scale, 8 * scale);
    }
    let mut tm = TEXTMETRICW::default();
    // SAFETY: `hdc` is the screen DC just obtained and `tm` lives for the whole call. The
    // previously selected font is restored before the DC is released.
    let measured = unsafe {
        let previous = SelectObject(hdc, HGDIOBJ(font.0));
        let ok = GetTextMetricsW(hdc, &mut tm).as_bool();
        SelectObject(hdc, previous);
        ReleaseDC(None, hdc);
        ok
    };
    if measured {
        (
            tm.tmHeight + 8 * scale,
            (tm.tmHeight + tm.tmExternalLeading).max(1),
            tm.tmAveCharWidth.max(1),
        )
    } else {
        (22 * scale, 16 * scale, 8 * scale)
    }
}

/// The usable area of the monitor the user is working on, excluding the taskbar.
pub fn work_area() -> RECT {
    // SAFETY: no preconditions; a null foreground window resolves to the nearest monitor.
    let monitor = unsafe { MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` has its cbSize set as the API requires and lives for the whole call.
    let have = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
    if have {
        info.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    }
}

/// Where to put a window of this size, on the monitor the user is working on.
///
/// Slightly above centre: the eye goes there first, and it is less likely to sit on top of the
/// text being corrected than dead centre is.
pub fn centre_on_active_monitor(width: i32, height: i32) -> (i32, i32) {
    place_in(work_area(), width, height)
}

/// The same placement within a work area the caller has already fetched.
pub fn place_in(work: RECT, width: i32, height: i32) -> (i32, i32) {
    (
        work.left + (work.right - work.left - width) / 2,
        work.top + (work.bottom - work.top - height) / 3,
    )
}

/// Take the foreground, working around the restriction that normally prevents it.
///
/// `SetForegroundWindow` alone is not enough and its failure is invisible in the worst way: the
/// window is still *activated* within its own thread, so it receives `WM_ACTIVATE` and looks
/// entirely healthy, while every keystroke continues to go to whatever kept the foreground.
/// Measured, not assumed — the first scripted run of `probe_chooser` logged `WM_ACTIVATE` and
/// then not one `WM_KEYDOWN` out of six keys sent.
///
/// Attaching this thread's input queue to the foreground window's thread makes the permitted
/// case true for the duration of the call. The attachment is dropped immediately: while it is
/// in place the two threads share focus and keyboard state, which is not a condition to leave
/// running.
///
/// Retried, because losing a race against another window activating is recoverable where the
/// restriction itself is not. Returns whether the window really ended up in front, so a caller
/// can refuse rather than present a window that cannot be typed into — one of these windows not
/// in the foreground has no way out, since its own Escape would go elsewhere.
pub fn take_foreground(hwnd: HWND) -> bool {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        // SAFETY: no preconditions. A null foreground window yields thread id 0, which the
        // guard below treats as "nothing to attach to".
        let their_thread = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), None) };
        // SAFETY: no preconditions.
        let our_thread = unsafe { GetCurrentThreadId() };

        let attached = their_thread != 0
            && their_thread != our_thread
            // SAFETY: both thread ids are live for the call — ours by definition, theirs
            // because it owns the window currently in front. A refusal is reported as a bool.
            && unsafe { AttachThreadInput(our_thread, their_thread, true) }.as_bool();

        // SAFETY: `hwnd` is alive and owned by this thread. Each call reports failure as a
        // bool, and the check below is what decides whether any of it worked.
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            let _ = SetFocus(Some(hwnd));
        }

        if attached {
            // SAFETY: detaching the attachment made above, with the same two thread ids.
            unsafe {
                let _ = AttachThreadInput(our_thread, their_thread, false);
            }
        }

        // SAFETY: no preconditions.
        let now = unsafe { GetForegroundWindow() };
        if now == hwnd {
            return true;
        }
    }
    false
}
