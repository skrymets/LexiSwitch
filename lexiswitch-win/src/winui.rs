//! The few window mechanics LexiSwitch's small windows all need.
//!
//! Three of them now exist, built the same way for the same reasons: they take focus because
//! [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md) leaves no other way to read a
//! keyboard, and they draw with raw GDI because the 5 MB budget rules out a toolkit.
//!
//! Everything here is shared rather than copied, and one item is here specifically because
//! copying it failed once. The rule that a window must settle in the foreground before losing
//! activation counts as the user walking away lived in the chooser's own state; the recorder
//! reproduced the bug in full when it was written, cancelling itself instantly with nothing on
//! screen. It is now [`Activation`], which cannot be used without arming it, because arming is
//! what [`Activation::present`] does.

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, DrawFrameControl, DrawTextW, FillRect, GetDC,
    GetDeviceCaps, DFCS_BUTTONCHECK, DFCS_CHECKED, DFC_BUTTON,
    GetMonitorInfoW, GetSysColor, GetTextMetricsW, MonitorFromWindow, ReleaseDC, SelectObject,
    SetTextColor, CLEARTYPE_QUALITY, COLOR_BTNFACE, DEFAULT_CHARSET, DRAW_TEXT_FORMAT,
    DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, FF_DONTCARE,
    FONT_CLIP_PRECISION, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, HGDIOBJ, LOGPIXELSY, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, OUT_TT_PRECIS, TEXTMETRICW,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    ShowWindow, SW_SHOW, WA_INACTIVE,
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
fn ui_font(scale: i32, emphasised: bool) -> HFONT {
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
fn metrics(font: HFONT, scale: i32) -> (i32, i32, i32) {
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

/// A copy the user made from inside a transaction window.
///
/// Carried back to the caller so the transaction can be told that this, and not the contents
/// from before it started, is what should be on the clipboard when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copied {
    pub text: String,
    /// Whether Windows Clipboard History may keep it.
    pub exposable: bool,
}

/// Put a result on the clipboard, honouring the exposure decision.
///
/// Returns whether it worked. A window uses that to say `Copied` only when it is true — this
/// project does not report success it has not observed, and a clipboard that another process is
/// holding open is exactly the case where an optimistic message would be wrong.
///
/// Writing to the clipboard is the one thing these windows may do besides draw. They still
/// cannot reach the document: they are not given anything that could.
pub fn copy_result(text: &str, exposable: bool, owner: HWND, attempts: u32) -> bool {
    let Ok(clip) = crate::clipboard::Clipboard::open(owner, attempts) else {
        return false;
    };
    crate::clipboard::Restore::kept(text.to_string(), exposable)
        .write(&clip)
        .is_ok()
}

/// Whether a window has settled in the foreground, and what losing it means.
///
/// Exists as a type rather than a `bool` field because it was a `bool` field twice and was
/// wrong the second time. Taking the foreground from another process emits activate,
/// deactivate, activate before the user has done anything; a window that treats the first
/// deactivation as a departure cancels itself before its message loop starts, which is what
/// made the chooser flash and vanish and then made the recorder do the same.
///
/// The rule is not stated separately from the act of showing the window: [`present`] is the
/// only way to get an armed `Activation`, so a window that shows itself correctly cannot fail
/// to arm, and one that never came to the front can never report a departure.
///
/// [`present`]: Activation::present
#[derive(Debug, Default)]
pub struct Activation {
    armed: bool,
}

impl Activation {
    /// A window that has not been shown yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show `hwnd`, bring it to the foreground, and arm the departure rule.
    ///
    /// Returns false when the foreground could not be taken, and leaves the rule unarmed. A
    /// window that is not in front receives no keys and its modal loop has no way out — its own
    /// Escape would go to whatever kept the foreground — so refusing is the only outcome that is
    /// not a hang, and the caller is expected to destroy the window and report it.
    ///
    /// Retried, because losing a race against another window activating is recoverable.
    pub fn present(&mut self, hwnd: HWND) -> bool {
        // SAFETY: `hwnd` is a live window owned by the calling thread.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        for attempt in 0..3 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            let took = take_foreground(hwnd);
            #[cfg(feature = "chooser-diag")]
            eprintln!("[winui] present attempt {attempt} -> {took}");
            if took {
                // Only now is losing the foreground meaningful: everything before this point is
                // the handoff settling, and reacting to it abandons something the user never did.
                self.armed = true;
                return true;
            }
        }
        false
    }

    /// Whether this `WM_ACTIVATE` means the user went somewhere else.
    ///
    /// `wparam` is the message's raw `WPARAM`. False before the window has settled in front, and
    /// false for a deactivation that leaves it in front anyway — that is a transient of the
    /// handoff, not a departure, and it is confirmed against the window manager rather than
    /// trusted from the message.
    pub fn is_departure(&self, hwnd: HWND, wparam: usize) -> bool {
        if !self.armed || (wparam & 0xFFFF) != WA_INACTIVE as usize {
            return false;
        }
        // SAFETY: no preconditions.
        let front = unsafe { GetForegroundWindow() };
        front != hwnd
    }
}

/// The fonts and measurements a window draws with, and the drawing itself.
///
/// Owns both fonts and deletes them when dropped, so the one cleanup step that has no visible
/// symptom when forgotten is not something a new window has to remember. Every measurement is
/// taken once here, because a layout that advances by measured metrics must be sized from the
/// same ones — a guess in one place and a measurement in the other is what clipped the
/// chooser's footer.
pub struct Pen {
    font: HFONT,
    bold: HFONT,
    /// Height of one row: the font's height plus breathing room.
    pub line_height: i32,
    /// Height of one *wrapped* line as `DrawTextW` lays it out — the font's height plus its
    /// external leading, with none of the row padding.
    ///
    /// Kept apart from `line_height` because scrolling and clipping work against lines GDI
    /// positions, not rows the caller positions. Using the padded value there steps the scroll
    /// further than one line and puts the clip boundary through the middle of a line, leaving a
    /// half-drawn row that reads as corruption.
    pub text_line: i32,
    /// Approximate width of one character, for turning a pixel width into a character count.
    pub char_width: i32,
    pub scale: i32,
}

impl Pen {
    /// Create both fonts and measure them.
    pub fn new(scale: i32) -> Self {
        let font = ui_font(scale, false);
        let bold = ui_font(scale, true);
        let (line_height, text_line, char_width) = metrics(font, scale);
        Self {
            font,
            bold,
            line_height,
            text_line,
            char_width,
            scale,
        }
    }

    /// The regular face, for selecting into a device context around a block of drawing.
    pub fn face(&self) -> HFONT {
        self.font
    }

    /// One line of text in a fixed box, clipped with an ellipsis if it overflows.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        hdc: HDC,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        colour: COLORREF,
        emphasised: bool,
    ) {
        let face = if emphasised { self.bold } else { self.font };
        // SAFETY: `hdc` is valid for the paint and the previous font is restored immediately.
        let previous = unsafe { SelectObject(hdc, HGDIOBJ(face.0)) };
        self.write(
            hdc,
            text,
            x,
            y,
            width,
            colour,
            DT_LEFT | DT_END_ELLIPSIS,
        );
        // SAFETY: restoring the object selected a moment ago.
        unsafe {
            SelectObject(hdc, previous);
        }
    }

    /// One line of text against the right edge of the box.
    pub fn draw_right(&self, hdc: HDC, text: &str, pad: i32, y: i32, inner: i32, colour: COLORREF) {
        self.write(hdc, text, pad, y, inner, colour, DT_RIGHT | DT_END_ELLIPSIS);
    }

    /// One line of text with an alignment the caller chooses and no ellipsis.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_aligned(
        &self,
        hdc: HDC,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        colour: COLORREF,
        align: DRAW_TEXT_FORMAT,
    ) {
        self.write(hdc, text, x, y, width, colour, align);
    }

    /// A real checkbox, drawn by Windows.
    ///
    /// `DrawFrameControl` rather than a `[x]` in text or a `☑` glyph: it is the control Windows
    /// draws everywhere else, it needs no font to contain the character, and it reads as a
    /// checkbox at a glance instead of as punctuation. Returns how much width it used, so the
    /// label after it can be placed without guessing.
    pub fn checkbox(&self, hdc: HDC, x: i32, y: i32, checked: bool) -> i32 {
        let size = (self.line_height - 8 * self.scale).clamp(11 * self.scale, 16 * self.scale);
        let mut rect = RECT {
            left: x,
            // Centred against the text line rather than sitting on it, or a row of boxes and
            // words reads as two rows.
            top: y + (self.line_height - size) / 2,
            right: x + size,
            bottom: y + (self.line_height - size) / 2 + size,
        };
        let mut state = DFCS_BUTTONCHECK;
        if checked {
            state |= DFCS_CHECKED;
        }
        // SAFETY: `rect` lives for the whole call and `hdc` is valid for this paint. A failure
        // is reported as a bool and leaves the area undrawn, which is a missing box rather than
        // anything unsound.
        unsafe {
            let _ = DrawFrameControl(hdc, &mut rect, DFC_BUTTON, state);
        }
        size
    }

    /// A horizontal separator in the system's control colour.
    pub fn rule(&self, hdc: HDC, left: i32, y: i32, right: i32) {
        let rect = RECT {
            left,
            top: y,
            right,
            bottom: y + 1.max(self.scale),
        };
        // SAFETY: `hdc` is valid for the paint and the brush is deleted immediately.
        unsafe {
            let brush = CreateSolidBrush(COLORREF(GetSysColor(COLOR_BTNFACE)));
            FillRect(hdc, &rect, brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));
        }
    }

    /// The one call every helper above funnels into.
    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        hdc: HDC,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        colour: COLORREF,
        format: DRAW_TEXT_FORMAT,
    ) {
        // Nothing to draw is not the same as drawing nothing. `DrawTextW` takes a slice, and an
        // empty `Vec<u16>` has a dangling pointer — passing it faults *inside the window
        // procedure*, which Windows answers by killing the process with
        // STATUS_FATAL_USER_CALLBACK_EXCEPTION. Not a Rust panic, so nothing catches it and no
        // message is printed; the window is simply half drawn and then the agent is gone.
        //
        // Reached by selecting only whitespace: every identifier style renders to an empty
        // string, and its preview is the first empty text any of these windows had ever been
        // asked to draw.
        if text.is_empty() {
            return;
        }

        let mut rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + self.line_height,
        };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: `hdc` is valid for the paint, and `wide` and `rect` live for the whole call.
        unsafe {
            SetTextColor(hdc, colour);
            DrawTextW(
                hdc,
                &mut wide,
                &mut rect,
                DT_SINGLELINE | DT_NOPREFIX | format,
            );
        }
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        // SAFETY: both fonts were created by `new` and belong to this `Pen`. A `Pen` outlives the
        // window that draws with it — the window is destroyed before the state holding the `Pen`
        // is dropped — so neither is still selected into any device context.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            let _ = DeleteObject(HGDIOBJ(self.bold.0));
        }
    }
}
