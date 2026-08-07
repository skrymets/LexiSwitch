//! The window that asks which conversion the user meant.
//!
//! The pivot's central promise is that nothing is modified before an explicit commit
//! ([ADR-0012](../../docs/adr/0012-single-chooser-transaction.md)). This is where that promise
//! is kept: the selection has already been read into a
//! [`Transaction`](crate::transaction::Transaction), every candidate has already been rendered
//! from it, and this window does nothing but display them and return a decision. It cannot
//! modify the document because it is not given anything that could.
//!
//! ## Why it takes focus
//!
//! A keyboard-driven chooser has to receive keys. The only way to read them without focus is a
//! low-level keyboard hook, which [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md)
//! forbids outright, and registering Up/Down/Enter/Escape as global hotkeys is not a serious
//! substitute. So the chooser activates, the user's application loses the foreground, and
//! ending the transaction hands it back. That the selection survives that round trip was
//! measured before this was written
//! ([findings](../../docs/spikes/2026-08-07-focus-round-trip.md)).
//!
//! ## Why it runs on the worker thread
//!
//! Windows are thread-affine: messages go to the thread that created them. Running a modal loop
//! here would freeze whichever loop it ran on, and the main loop is what keeps the tray alive —
//! so this runs on the worker, the thread that is already allowed to be slow. The main loop
//! stays responsive throughout, and the tray menu still opens while a chooser is on screen.
//!
//! ## Drawing
//!
//! Raw GDI, because the single-executable requirement and the 5 MB budget rule out every
//! toolkit that would pull in a graphics stack (design.md). Colours come from the system
//! palette rather than being hard-coded, so a dark theme is inherited rather than fought.

use std::ffi::c_void;

use lexiswitch_core::candidates::Candidate;
use lexiswitch_core::preview::compact;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetDC, GetDeviceCaps, GetMonitorInfoW, GetSysColor, GetTextMetricsW, InvalidateRect,
    MonitorFromWindow, ReleaseDC, SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY,
    COLOR_BTNFACE,
    COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT,
    DEFAULT_CHARSET, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_WORDBREAK,
    FF_DONTCARE, FONT_CLIP_PRECISION, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, HGDIOBJ, LOGPIXELSY,
    MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_TT_PRECIS, PAINTSTRUCT, TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetForegroundWindow, GetMessageW, GetWindowLongPtrW, GetWindowThreadProcessId, LoadCursorW,
    RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    CREATESTRUCTW, GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE,
    WM_KEYDOWN, WM_NCCREATE, WM_PAINT, WNDCLASSW, WS_BORDER, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchChooser");

/// Virtual-key codes, as plain integers because that is what `WM_KEYDOWN` delivers.
const VK_ESCAPE_U: usize = 0x1B;
const VK_RETURN_U: usize = 0x0D;
const VK_SPACE_U: usize = 0x20;
const VK_PRIOR_U: usize = 0x21;
const VK_NEXT_U: usize = 0x22;
const VK_UP_U: usize = 0x26;
const VK_DOWN_U: usize = 0x28;
const VK_HOME_U: usize = 0x24;
const VK_END_U: usize = 0x23;

/// How the user ended the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Replace the selection with this candidate, leaving the active layout alone.
    Apply { index: usize },
    /// Replace it and additionally request the candidate's target layout.
    ApplyAndSwitch { index: usize },
    /// Change nothing.
    Cancel,
}

/// Everything the window draws and everything the keyboard changes.
///
/// Lives on the stack of [`show`] for exactly as long as the window does. The window procedure
/// reaches it through `GWLP_USERDATA`; see the SAFETY comment there for why that is sound.
struct Chooser<'a> {
    original: &'a str,
    candidates: &'a [Candidate],
    /// The layout in force in the focused window, shown as context. Deliberately *not* labelled
    /// as any candidate's source: it is a fact about the present, and reading it as evidence
    /// about text typed earlier is the defect this whole change removes.
    active: Option<&'a str>,
    selected: usize,
    /// Whether the selected candidate is shown in full instead of the list.
    expanded: bool,
    /// First visible line of the expanded view.
    scroll: usize,
    decision: Option<Decision>,
    /// Set once the window has been activated, so the initial transition to foreground is not
    /// mistaken for the user clicking away.
    activated: bool,
    font: HFONT,
    bold: HFONT,
    /// Pixel height of one line of text, measured from the font rather than assumed.
    line_height: i32,
    /// Approximate pixel width of one character, for turning a pixel width into the character
    /// count [`compact`] works in.
    char_width: i32,
    scale: i32,
}

/// Open the chooser and block until the user decides.
///
/// Returns [`Decision::Cancel`] for every way out that is not an explicit commit — Escape, the
/// window closing, or focus moving elsewhere — because the transaction's default has to be
/// "change nothing" for the promise above to mean anything.
///
/// `candidates` must not be empty. A selection that yields nothing is a distinct outcome the
/// caller reports without opening a window; there is nothing here for the user to choose
/// between.
pub fn show(
    original: &str,
    candidates: &[Candidate],
    active: Option<&str>,
) -> Result<Decision, WinError> {
    debug_assert!(!candidates.is_empty(), "the caller reports the empty case");
    if candidates.is_empty() {
        return Ok(Decision::Cancel);
    }

    // SAFETY: the class name is a static wide string with a NUL and the WNDCLASSW is fully
    // initialised before registration. A zero return means the class is already registered by
    // an earlier invocation, which is harmless — CreateWindowExW below is the real check.
    let hinstance =
        unsafe { GetModuleHandleW(None) }.map_err(|e| WinError::new("GetModuleHandleW", e))?;
    // SAFETY: IDC_ARROW is a built-in cursor identifier, valid with a null module handle.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let class = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance.into(),
        lpszClassName: CLASS_NAME,
        hCursor: cursor,
        ..Default::default()
    };
    // SAFETY: `class` is fully initialised and its name outlives the call; re-registration
    // failure is ignored deliberately, as documented above.
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let scale = dpi_scale();
    let (font, bold) = fonts(scale);
    let (line_height, char_width) = metrics(font, scale);
    let mut state = Chooser {
        original,
        candidates,
        active,
        selected: 0,
        expanded: false,
        scroll: 0,
        decision: None,
        activated: false,
        font,
        bold,
        line_height,
        char_width,
        scale,
    };

    let (x, y, width, height) = geometry(&state);

    // The state pointer is handed to CreateWindowExW, which passes it to WM_NCCREATE before
    // returning. From there the window procedure holds it for the window's whole life.
    let param = &mut state as *mut Chooser as *const c_void;

    // SAFETY: the class is registered, the strings are static and NUL-terminated, and `param`
    // points to a live `Chooser` that outlives this window — `state` is dropped only after
    // DestroyWindow below, and no other code path leaves this function in between.
    let hwnd = unsafe {
        CreateWindowExW(
            // Out of the taskbar and Alt-Tab, and above the application being corrected — a
            // chooser behind the window it is about would be useless.
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            CLASS_NAME,
            w!("LexiSwitch"),
            WS_POPUP | WS_BORDER,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(hinstance.into()),
            Some(param),
        )
    }
    .map_err(|e| WinError::new("CreateWindowExW", e))?;

    // SAFETY: `hwnd` was just created by this function and is still alive.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
    // A chooser that is not in front receives no keys, and its modal loop has no way out —
    // Escape would go to whatever kept the foreground. Refusing is the only outcome that is
    // not a hang, and it leaves the transaction to be cancelled cleanly by the caller.
    // Retried because losing a race against another window activating is recoverable, unlike
    // the restriction itself.
    let mut foreground = false;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        if force_foreground(hwnd) {
            foreground = true;
            break;
        }
    }
    if !foreground {
        // SAFETY: `hwnd` was created above and has not been destroyed elsewhere.
        unsafe {
            let _ = DestroyWindow(hwnd);
            let _ = DeleteObject(HGDIOBJ(font.0));
            let _ = DeleteObject(HGDIOBJ(bold.0));
        }
        return Err(WinError::last("SetForegroundWindow"));
    }

    // A modal loop of this window's own. It runs on the worker thread, so the main loop — and
    // with it the tray — keeps running while the chooser is open.
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid MSG for the whole call. 0 is WM_QUIT and -1 is an error;
        // both end the loop, as does a decision having been made.
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        // SAFETY: `msg` was filled by GetMessageW and is untouched since.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if state.decision.is_some() {
            break;
        }
    }

    // SAFETY: `hwnd` was created here and has not been destroyed elsewhere. It must go before
    // `state` does, because the window procedure dereferences the pointer to it.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    // SAFETY: both fonts were created by `fonts` and are no longer selected into any DC — the
    // paint handler restores the previous object before returning.
    unsafe {
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteObject(HGDIOBJ(bold.0));
    }

    Ok(state.decision.unwrap_or(Decision::Cancel))
}

/// Take the foreground, working around the restriction that normally prevents it.
///
/// `SetForegroundWindow` alone is not enough and its failure is invisible in the worst way: the
/// window is still *activated* within its own thread, so it receives `WM_ACTIVATE` and looks
/// entirely healthy, while every keystroke continues to go to whatever kept the foreground.
/// Measured, not assumed — the first scripted run of `probe_chooser` logged `WM_ACTIVATE` and
/// then not one `WM_KEYDOWN` out of six keys sent.
///
/// Windows grants the foreground to a process that owns the current foreground window, among
/// other cases. Attaching this thread's input queue to the foreground window's thread makes
/// that true for the duration of the call, which is the long-established way through. The
/// attachment is dropped immediately: while it is in place the two threads share focus and
/// keyboard state, which is not a condition to leave running.
///
/// Returns whether the window really ended up in front, so a caller can say so rather than
/// presenting a window that cannot be typed into.
fn force_foreground(hwnd: HWND) -> bool {
    // SAFETY: no preconditions. A null foreground window yields thread id 0, which the guard
    // below treats as "nothing to attach to".
    let their_thread = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), None) };
    // SAFETY: no preconditions.
    let our_thread = unsafe { GetCurrentThreadId() };

    let attached = their_thread != 0
        && their_thread != our_thread
        // SAFETY: both thread ids are live for the call — ours by definition, theirs because it
        // owns the window that is currently in front. A refusal is reported as a bool.
        && unsafe { AttachThreadInput(our_thread, their_thread, true) }.as_bool();

    // SAFETY: `hwnd` is alive and owned by this thread. Each call reports failure as a bool,
    // and the check below is what decides whether any of it worked.
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
    now == hwnd
}

/// Pixels per logical unit at the primary display's DPI, as a whole-number scale.
///
/// Read from the device context rather than `GetDpiForWindow` so no window is needed and no
/// further windows-rs feature is pulled in for it.
fn dpi_scale() -> i32 {
    // SAFETY: a null HDC asks GetDeviceCaps about the primary display, which is what is wanted
    // before any window exists.
    let dpi = unsafe { GetDeviceCaps(None, LOGPIXELSY) };
    if dpi <= 0 {
        1
    } else {
        (dpi / 96).max(1)
    }
}

/// The regular and emphasised faces, sized for the display.
fn fonts(scale: i32) -> (HFONT, HFONT) {
    let height = -(11 * 96 * scale / 72);
    let make = |weight: i32| {
        // SAFETY: every argument is a plain value and the face name is a static NUL-terminated
        // wide string. A failure returns a null HFONT, which SelectObject tolerates by leaving
        // the DC's current font in place.
        unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                FONT_CLIP_PRECISION(0),
                CLEARTYPE_QUALITY,
                FF_DONTCARE.0 as u32,
                // Segoe UI covers Cyrillic, which is most of what this window renders.
                w!("Segoe UI"),
            )
        }
    };
    (make(FW_NORMAL.0 as i32), make(FW_SEMIBOLD.0 as i32))
}

/// The height of one line and the width of one character, for the font that will be used.
///
/// Measured against a screen device context *before* the window exists, because the window has
/// to be sized from these numbers. Measuring lazily during the first paint is what clipped the
/// footer: the layout advanced by the font's real line height while the window had been sized
/// with a guess, so the last line fell outside the client area and the target layout that
/// task 3.7 requires be named was drawn off the bottom edge.
fn metrics(font: HFONT, scale: i32) -> (i32, i32) {
    // SAFETY: a null window handle asks for a DC covering the whole screen, which is a valid
    // request and needs no window to exist yet.
    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return (22 * scale, 8 * scale);
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
        (tm.tmHeight + 8 * scale, tm.tmAveCharWidth.max(1))
    } else {
        (22 * scale, 8 * scale)
    }
}

/// Where the window goes and how big it is.
///
/// Centred on the monitor holding the foreground window, per design.md — near-caret placement
/// is deliberately deferred, because it needs caret geometry, edge repositioning and an overlay
/// that would cover the very text under review.
fn geometry(state: &Chooser) -> (i32, i32, i32, i32) {
    let scale = state.scale;
    let width = 760 * scale;
    let pad = 14 * scale;
    let line = state.line_height;
    let rows = state.candidates.len() as i32;
    // Every term matches a step the paint takes, in the same order, so the two cannot drift:
    // padding, title, gap, original, gap, rule, gap, rows, rule, gap, two footer lines, padding.
    let height = pad
        + line
        + 4 * scale
        + line
        + 6 * scale
        + 1.max(scale)
        + 8 * scale
        + rows * line
        + 4 * scale
        + 1.max(scale)
        + 8 * scale
        + line * 2
        + pad;

    // SAFETY: no preconditions; a null foreground window resolves to the nearest monitor,
    // which for a null handle is the primary one.
    let monitor = unsafe { MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` has its cbSize set as the API requires and lives for the whole call.
    let have = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
    let work = if have {
        info.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    };

    let width = width.min(work.right - work.left - 40 * scale);
    let height = height.min(work.bottom - work.top - 40 * scale);
    let x = work.left + (work.right - work.left - width) / 2;
    // Slightly above centre: the eye goes there first, and it is less likely to sit on top of
    // the text being corrected than dead centre is.
    let y = work.top + (work.bottom - work.top - height) / 3;
    (x, y, width, height)
}

impl Chooser<'_> {
    /// Apply a key press. Returns whether anything changed and the window needs repainting.
    fn key(&mut self, vk: usize, ctrl: bool) -> bool {
        match vk {
            VK_ESCAPE_U => {
                self.decision = Some(Decision::Cancel);
                false
            }
            VK_RETURN_U => {
                self.decision = Some(if ctrl {
                    Decision::ApplyAndSwitch {
                        index: self.selected,
                    }
                } else {
                    Decision::Apply {
                        index: self.selected,
                    }
                });
                false
            }
            VK_SPACE_U => {
                self.expanded = !self.expanded;
                self.scroll = 0;
                true
            }
            // While expanded the arrows scroll the text; the selection is not moving, so the
            // candidate under review cannot change out from under the user mid-read.
            VK_UP_U if self.expanded => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            VK_DOWN_U if self.expanded => {
                self.scroll += 1;
                true
            }
            VK_PRIOR_U if self.expanded => {
                self.scroll = self.scroll.saturating_sub(5);
                true
            }
            VK_NEXT_U if self.expanded => {
                self.scroll += 5;
                true
            }
            VK_HOME_U if self.expanded => {
                self.scroll = 0;
                true
            }
            // Wrapping, because a list this short is faster to cycle than to bound-check by
            // eye, and because two candidates is the common case.
            VK_UP_U => {
                self.selected = if self.selected == 0 {
                    self.candidates.len() - 1
                } else {
                    self.selected - 1
                };
                true
            }
            VK_DOWN_U => {
                self.selected = (self.selected + 1) % self.candidates.len();
                true
            }
            VK_HOME_U => {
                self.selected = 0;
                true
            }
            VK_END_U => {
                self.selected = self.candidates.len() - 1;
                true
            }
            _ => false,
        }
    }

    /// Draw the whole window.
    fn paint(&self, hdc: HDC, client: RECT) {
        let scale = self.scale;
        let pad = 14 * scale;

        // SAFETY: `hdc` is the device context BeginPaint returned and is valid until EndPaint.
        // Every object selected here is restored or is a stock value.
        unsafe {
            let bg = CreateSolidBrush(COLORREF(GetSysColor(COLOR_WINDOW)));
            FillRect(hdc, &client, bg);
            let _ = DeleteObject(HGDIOBJ(bg.0));
            SetBkMode(hdc, TRANSPARENT);
        }

        // SAFETY: as above; the previous font is restored at the end of this function.
        let previous = unsafe { SelectObject(hdc, HGDIOBJ(self.font.0)) };

        let text_colour = COLORREF(
            // SAFETY: no preconditions; returns a colour for a documented system index.
            unsafe { GetSysColor(COLOR_WINDOWTEXT) },
        );
        // SAFETY: as above.
        let dim = COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) });

        let mut y = pad;
        let full = client.right - client.left;
        let inner = full - pad * 2;

        // --- header ------------------------------------------------------------------
        self.draw(hdc, "Convert selection", pad, y, inner, text_colour, true);
        if let Some(active) = self.active {
            // Labelled "Typing in", never "from": the active layout is context, and presenting
            // it as any candidate's source is precisely the confusion the spec forbids.
            self.draw_right(
                hdc,
                &format!("Typing in {active}"),
                pad,
                y,
                inner,
                dim,
            );
        }
        y += self.line_height + 4 * scale;

        let chars = (inner / self.char_width).max(8) as usize;
        let original = compact(self.original, self.original, chars.saturating_sub(10));
        self.draw(
            hdc,
            &format!("Selected  {}", decorate(&original)),
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.line_height + 6 * scale;
        self.rule(hdc, pad, y, full - pad);
        y += 8 * scale;

        // --- body --------------------------------------------------------------------
        if self.expanded {
            y = self.paint_expanded(hdc, pad, y, inner, client, text_colour, dim);
        } else {
            y = self.paint_list(hdc, pad, y, inner, full, text_colour, dim);
        }

        // --- footer ------------------------------------------------------------------
        self.rule(hdc, pad, y, full - pad);
        y += 8 * scale;
        let target = &self.candidates[self.selected].to;
        self.draw(
            hdc,
            &format!(
                "\u{2191}\u{2193} {}    Enter apply    Space {}    Esc cancel",
                if self.expanded { "scroll" } else { "choose" },
                if self.expanded { "collapse" } else { "expand" }
            ),
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.line_height;
        // Task 3.7: the layout a switching commit would activate is named before the user
        // commits, not discovered afterwards.
        self.draw(
            hdc,
            &format!("Ctrl+Enter apply and switch typing to {target}"),
            pad,
            y,
            inner,
            dim,
            false,
        );

        // SAFETY: restoring the object that was selected before this function ran.
        unsafe {
            SelectObject(hdc, previous);
        }
    }

    /// One row per candidate.
    #[allow(clippy::too_many_arguments)]
    fn paint_list(
        &self,
        hdc: HDC,
        pad: i32,
        mut y: i32,
        inner: i32,
        full: i32,
        text_colour: COLORREF,
        dim: COLORREF,
    ) -> i32 {
        let scale = self.scale;
        // Fixed columns so the layout names line up and the eye compares results, not labels.
        let label_width = 150 * scale;
        let count_width = 130 * scale;

        for (i, candidate) in self.candidates.iter().enumerate() {
            let selected = i == self.selected;
            if selected {
                let row = RECT {
                    left: pad - 6 * scale,
                    top: y - 3 * scale,
                    right: full - pad + 6 * scale,
                    bottom: y + self.line_height,
                };
                // SAFETY: `hdc` is valid for the paint and the brush is deleted immediately.
                unsafe {
                    let brush = CreateSolidBrush(COLORREF(GetSysColor(COLOR_HIGHLIGHT)));
                    FillRect(hdc, &row, brush);
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                }
            }
            let on_row = if selected {
                // SAFETY: no preconditions; returns a colour for a documented system index.
                COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHTTEXT) })
            } else {
                text_colour
            };
            let secondary = if selected { on_row } else { dim };

            self.draw(
                hdc,
                &format!("{} \u{2192} {}", candidate.from, candidate.to),
                pad,
                y,
                label_width,
                secondary,
                false,
            );

            // Every row is rendered from the original selection, which is what `generate`
            // already guarantees — no candidate is derived from another's result, so moving
            // back and forth cannot compound.
            let room = (inner - label_width - count_width) / self.char_width;
            let preview = compact(&candidate.text, self.original, (room.max(8)) as usize);
            self.draw(
                hdc,
                &decorate(&preview),
                pad + label_width,
                y,
                inner - label_width - count_width,
                on_row,
                selected,
            );

            // Task 3.4: a partial conversion that looks complete is how a user ends up with
            // half-corrected text and no idea, so the count is shown rather than inferred.
            if candidate.unchanged > 0 {
                self.draw_right(
                    hdc,
                    &format!("{} unchanged", candidate.unchanged),
                    pad,
                    y,
                    inner,
                    secondary,
                );
            }
            y += self.line_height;
        }
        y + 4 * scale
    }

    /// The selected candidate in full, wrapped and scrollable.
    #[allow(clippy::too_many_arguments)]
    fn paint_expanded(
        &self,
        hdc: HDC,
        pad: i32,
        y: i32,
        inner: i32,
        client: RECT,
        text_colour: COLORREF,
        dim: COLORREF,
    ) -> i32 {
        let scale = self.scale;
        let candidate = &self.candidates[self.selected];
        let bottom = client.bottom - (58 * scale);

        self.draw(
            hdc,
            &format!(
                "{} \u{2192} {}   (Space closes, \u{2191}\u{2193} scrolls)",
                candidate.from, candidate.to
            ),
            pad,
            y,
            inner,
            dim,
            false,
        );
        let top = y + self.line_height;

        // Scrolling by shifting the rectangle upward and letting DrawTextW clip is what keeps
        // this to one call: no line breaking of our own, and no wrapping logic to disagree
        // with the one GDI already applies.
        let mut rect = RECT {
            left: pad,
            top: top - (self.scroll as i32 * self.line_height),
            right: client.right - pad,
            bottom,
        };
        let mut wide: Vec<u16> = candidate.text.encode_utf16().collect();
        // SAFETY: `wide` and `rect` both live for the whole call. DT_WORDBREAK may rewrite the
        // buffer only under DT_MODIFYSTRING, which is not passed.
        unsafe {
            SetTextColor(hdc, text_colour);
            DrawTextW(hdc, &mut wide, &mut rect, DT_WORDBREAK | DT_NOPREFIX);
        }
        bottom + 4 * scale
    }

    /// One line of text in a fixed box, clipped with an ellipsis if it somehow overflows.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        hdc: HDC,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        colour: COLORREF,
        emphasised: bool,
    ) {
        let mut rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + self.line_height,
        };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let face = if emphasised { self.bold } else { self.font };
        // SAFETY: `hdc` is valid for the paint, `wide` and `rect` live for the whole call, and
        // the font is restored immediately afterwards.
        unsafe {
            let previous = SelectObject(hdc, HGDIOBJ(face.0));
            SetTextColor(hdc, colour);
            DrawTextW(
                hdc,
                &mut wide,
                &mut rect,
                DT_SINGLELINE | DT_LEFT | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            SelectObject(hdc, previous);
        }
    }

    /// One line of text against the right edge.
    fn draw_right(&self, hdc: HDC, text: &str, pad: i32, y: i32, inner: i32, colour: COLORREF) {
        let mut rect = RECT {
            left: pad,
            top: y,
            right: pad + inner,
            bottom: y + self.line_height,
        };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: as in `draw`.
        unsafe {
            SetTextColor(hdc, colour);
            DrawTextW(
                hdc,
                &mut wide,
                &mut rect,
                DT_SINGLELINE | DT_RIGHT | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
    }

    /// A horizontal separator.
    fn rule(&self, hdc: HDC, left: i32, y: i32, right: i32) {
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
}

/// Render a preview with its omissions marked.
///
/// The spec requires that where a preview omits content the omission is visible; a horizontal
/// ellipsis on the side content was dropped from says which side, which "…" in the middle
/// would not.
fn decorate(preview: &lexiswitch_core::preview::Preview) -> String {
    let mut out = String::new();
    if preview.elided_start {
        out.push('\u{2026}');
    }
    out.push_str(&preview.text);
    if preview.elided_end {
        out.push('\u{2026}');
    }
    out
}

/// SAFETY (contract on the caller, which is Windows): invoked by the window manager for a
/// window of this class. `lp` is a `CREATESTRUCTW` for `WM_NCCREATE` and is not dereferenced
/// for any other message.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: Windows documents `lp` as a pointer to a CREATESTRUCTW for this message, and
        // `lpCreateParams` is the pointer handed to CreateWindowExW — the `&mut Chooser` from
        // `show`, which outlives this window.
        let state = unsafe { (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams };
        // SAFETY: storing an integer in this window's user data slot; no dereference here.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        }
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }

    // SAFETY: reading back the integer stored above. Zero until WM_NCCREATE has run, which is
    // why the null check below is not optional — messages do arrive before it.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Chooser;
    if ptr.is_null() {
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }
    // SAFETY: the pointer was stored by WM_NCCREATE from `show`'s stack frame, which does not
    // return until after DestroyWindow. Windows delivers messages for one window on one thread
    // at a time, so no other reference to the same `Chooser` is live during this call.
    let state = unsafe { &mut *ptr };

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: `ps` lives for the whole paint and EndPaint is called with the same
            // window and structure, as the API requires.
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                state.paint(hdc, ps.rcPaint);
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            // SAFETY: no preconditions; reports the current state of one virtual key.
            let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            if state.key(wp.0, ctrl) {
                // SAFETY: invalidating the whole client area of a window that is alive for the
                // duration of this call.
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
            LRESULT(0)
        }
        // Clicking away is a cancellation, not an invitation to leave a chooser stranded on
        // screen holding a transaction open. The first activation is skipped because taking
        // the foreground produces one before the user has done anything.
        WM_ACTIVATE => {
            if (wp.0 & 0xFFFF) == WA_INACTIVE as usize {
                if state.activated && state.decision.is_none() {
                    state.decision = Some(Decision::Cancel);
                }
            } else {
                state.activated = true;
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            state.decision = Some(Decision::Cancel);
            LRESULT(0)
        }
        // SAFETY: forwarding the exact arguments Windows passed.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}
