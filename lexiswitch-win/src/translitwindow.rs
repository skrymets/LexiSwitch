//! The window that asks which transliteration the user wants.
//!
//! The third transaction, beside layout conversion and case conversion
//! ([ADR-0017](../../docs/adr/0017-transliteration-third-transaction.md)). Like the other two it
//! is handed text that has already been read and results that have already been rendered, and
//! it does nothing but display them and return a decision — it cannot modify the document
//! because it is not given anything that could.
//!
//! It is modelled on the layout chooser and shares only what is genuinely common: `winui` for
//! chrome, focus and copying, and `preview` for fitting a result into a row. The chooser is not
//! touched by this module, by the rule ADR-0014 set for the case window. It differs from the
//! chooser in three ways: every ruleset is listed every time, one that would change nothing is
//! dimmed rather than left out, and there is no `Ctrl+Enter` because there is no layout to
//! switch to.
//!
//! It takes focus and runs on the worker thread for the reasons `chooser.rs` gives.

use std::ffi::c_void;

use lexiswitch_core::preview::compact;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, GetSysColor,
    IntersectClipRect, InvalidateRect, RestoreDC, SaveDC, SelectObject, SetBkMode, SetTextColor,
    COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT,
    DT_CALCRECT, DT_NOPREFIX, DT_WORDBREAK, HDC, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetWindowLongPtrW, SetWindowPos,
    TranslateMessage, CREATESTRUCTW, GWLP_USERDATA, IDC_ARROW, MSG, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, WM_ACTIVATE, WM_CLOSE, WM_KEYDOWN, WM_NCCREATE, WM_PAINT, WNDCLASSW, WS_BORDER,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::winui;
use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchTransliteration");

/// How many wrapped lines the expanded view shows when the caller expresses no preference.
const EXPANDED_LINES: i32 = 10;

/// The window's width before scaling.
const WIDTH: i32 = 760;

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
const VK_C_U: usize = 0x43;
const VK_H_U: usize = 0x48;
const VK_INSERT_U: usize = 0x2D;

/// One ruleset's result, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The ruleset's name, as its file gives it.
    pub name: String,
    /// The ruleset's source and target language tags.
    pub source: String,
    pub target: String,
    /// What the selection becomes.
    pub text: String,
    /// Cyrillic letters the ruleset did not cover.
    pub untouched: usize,
}

/// How the user ended the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Replace the selection with this row's result.
    Apply { index: usize },
    /// Change nothing.
    Cancel,
}

/// Everything the window draws and everything the keyboard changes.
///
/// Lives on the stack of [`show`] for exactly as long as the window does. The window procedure
/// reaches it through `GWLP_USERDATA`; see the SAFETY comment there for why that is sound.
struct Window<'a> {
    original: &'a str,
    rows: &'a [Row],
    selected: usize,
    /// Whether the selected result is shown in full instead of the list.
    expanded: bool,
    /// Set when a toggle changed how tall the window needs to be.
    resized: bool,
    /// First visible line of the expanded view.
    scroll: usize,
    decision: Option<Decision>,
    /// Where the clipboard can be reached from.
    owner: HWND,
    attempts: u32,
    /// Whether a copy may enter Windows Clipboard History. Off every time the window opens.
    expose_copy: bool,
    /// What the user has copied, carried out so the transaction stops restoring over it.
    copied: Option<String>,
    /// Whether the last copy attempt succeeded. `None` until one has been tried.
    copy_worked: Option<bool>,
    activation: winui::Activation,
    pen: winui::Pen,
    expanded_lines: i32,
}

/// Open the window and block until the user decides.
///
/// Returns [`Decision::Cancel`] for every way out that is not an explicit commit, and for a
/// commit of a row that would change nothing — replacing text with itself is not a result.
pub fn show(
    original: &str,
    rows: &[Row],
    expanded_lines: i32,
    owner: HWND,
    attempts: u32,
) -> Result<(Decision, Option<winui::Copied>), WinError> {
    // SAFETY: no preconditions; a null module name asks for this executable's own handle.
    let hinstance =
        unsafe { GetModuleHandleW(None) }.map_err(|e| WinError::new("GetModuleHandleW", e))?;
    // SAFETY: IDC_ARROW is a built-in cursor identifier, valid with a null module handle.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let class = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance.into(),
        lpszClassName: CLASS_NAME,
        hCursor: cursor,
        hIcon: crate::icon::app_icon(crate::icon::Kind::Main, crate::icon::Size::Large)
            .unwrap_or_default(),
        ..Default::default()
    };
    // SAFETY: `class` is fully initialised and its name is a static wide string. A zero return
    // means the class is already registered by an earlier invocation, which is harmless —
    // CreateWindowExW below is the real check.
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let mut state = Window {
        original,
        rows,
        // The first row that does something. With none, the first row: there is still a list
        // to read, and every row says it would change nothing.
        selected: rows.iter().position(|r| r.text != original).unwrap_or(0),
        expanded: false,
        resized: false,
        scroll: 0,
        decision: None,
        owner,
        attempts,
        expose_copy: false,
        copied: None,
        copy_worked: None,
        activation: winui::Activation::new(),
        pen: winui::Pen::new(winui::dpi_scale()),
        expanded_lines: if expanded_lines > 0 {
            expanded_lines
        } else {
            EXPANDED_LINES
        },
    };

    let (x, y, width, height) = geometry(&state);

    // The state pointer is handed to CreateWindowExW, which passes it to WM_NCCREATE before
    // returning. From there the window procedure holds it for the window's whole life.
    let param = &mut state as *mut Window as *const c_void;

    // SAFETY: the class is registered, the strings are static and NUL-terminated, and `param`
    // points to a live `Window` that outlives this window — `state` is dropped only after
    // DestroyWindow below, and no other code path leaves this function in between.
    let hwnd = unsafe {
        CreateWindowExW(
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

    // A window that is not in front receives no keys, and its modal loop has no way out.
    if !state.activation.present(hwnd) {
        // SAFETY: `hwnd` was created above and has not been destroyed elsewhere.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return Err(WinError::last("SetForegroundWindow"));
    }

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

    let copied = state.copied.clone().map(|text| winui::Copied {
        text,
        exposable: state.expose_copy,
    });
    Ok((state.decision.unwrap_or(Decision::Cancel), copied))
}

/// Where the window goes and how big it is: centred on the work area, like the chooser.
fn geometry(state: &Window) -> (i32, i32, i32, i32) {
    let scale = state.pen.scale;
    let work = winui::work_area();
    let width = (WIDTH * scale).min(work.right - work.left - 40 * scale);
    let height = state
        .height_for(state.expanded)
        .min(work.bottom - work.top - 40 * scale);
    let (x, y) = winui::place_in(work, width, height);
    (x, y, width, height)
}

impl Window<'_> {
    /// How tall the window has to be in the given mode. Every term matches a step the paint
    /// takes, in the same order, so the two cannot drift.
    fn height_for(&self, expanded: bool) -> i32 {
        let scale = self.pen.scale;
        let pad = 14 * scale;
        let line = self.pen.line_height;
        let body = if expanded {
            line + self.expanded_lines * self.pen.text_line + line
        } else {
            (self.rows.len() as i32).max(1) * line
        };
        pad + line
            + 4 * scale
            + line
            + 6 * scale
            + 1.max(scale)
            + 8 * scale
            + body
            + 4 * scale
            + 1.max(scale)
            + 8 * scale
            + line * 2
            + pad
    }

    /// Whether a row's result differs from the selection.
    fn changes(&self, row: &Row) -> bool {
        row.text != self.original
    }

    /// Put the selected row's whole result on the clipboard — the same text a commit would
    /// write, never the shortened preview.
    fn copy_selected(&mut self) {
        let text = self.rows[self.selected].text.clone();
        let worked = winui::copy_result(&text, self.expose_copy, self.owner, self.attempts);
        self.copy_worked = Some(worked);
        self.copied = worked.then_some(text);
    }

    /// Apply a key press. Returns whether anything changed and the window needs repainting.
    fn key(&mut self, vk: usize, ctrl: bool) -> bool {
        if self.rows.is_empty() {
            if matches!(vk, VK_ESCAPE_U | VK_RETURN_U | VK_SPACE_U) {
                self.decision = Some(Decision::Cancel);
            }
            return false;
        }
        if ctrl && (vk == VK_C_U || vk == VK_INSERT_U) {
            self.copy_selected();
            return true;
        }
        if vk == VK_H_U && !ctrl {
            self.expose_copy = !self.expose_copy;
            if self.copied.is_some() {
                self.copy_selected();
            }
            return true;
        }
        match vk {
            VK_ESCAPE_U => {
                self.decision = Some(Decision::Cancel);
                false
            }
            // A row that changes nothing is dimmed, not disabled, so it can be read and copied;
            // committing it cancels instead of replacing the text with itself.
            VK_RETURN_U => {
                self.decision = Some(if self.changes(&self.rows[self.selected]) {
                    Decision::Apply {
                        index: self.selected,
                    }
                } else {
                    Decision::Cancel
                });
                false
            }
            VK_SPACE_U => {
                self.expanded = !self.expanded;
                self.scroll = 0;
                self.resized = true;
                true
            }
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
            VK_UP_U => {
                self.selected = if self.selected == 0 {
                    self.rows.len() - 1
                } else {
                    self.selected - 1
                };
                true
            }
            VK_DOWN_U => {
                self.selected = (self.selected + 1) % self.rows.len();
                true
            }
            VK_HOME_U | VK_PRIOR_U => {
                self.selected = 0;
                true
            }
            VK_END_U | VK_NEXT_U => {
                self.selected = self.rows.len() - 1;
                true
            }
            _ => false,
        }
    }

    /// Draw the whole window.
    fn paint(&mut self, hwnd: HWND, hdc: HDC) {
        let mut client = RECT::default();
        // SAFETY: `client` lives for the whole call and `hwnd` is alive while its own window
        // procedure runs.
        if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
            return;
        }
        let scale = self.pen.scale;
        let pad = 14 * scale;

        // SAFETY: `hdc` is the device context BeginPaint returned and is valid until EndPaint.
        // The brush is deleted immediately after use.
        unsafe {
            let bg = CreateSolidBrush(COLORREF(GetSysColor(COLOR_WINDOW)));
            FillRect(hdc, &client, bg);
            let _ = DeleteObject(HGDIOBJ(bg.0));
            SetBkMode(hdc, TRANSPARENT);
        }

        // SAFETY: as above; the previous font is restored at the end of this function.
        let previous = unsafe { SelectObject(hdc, HGDIOBJ(self.pen.face().0)) };

        // SAFETY: no preconditions; returns a colour for a documented system index.
        let text_colour = COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) });
        // SAFETY: as above.
        let dim = COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) });

        let mut y = pad;
        let full = client.right - client.left;
        let inner = full - pad * 2;

        // --- header ------------------------------------------------------------------
        self.pen.draw(hdc, "Transliterate selection", pad, y, inner, text_colour, true);
        y += self.pen.line_height + 4 * scale;

        let chars = (inner / self.pen.char_width).max(8) as usize;
        let original = compact(self.original, self.original, chars.saturating_sub(10));
        self.pen.draw(
            hdc,
            &format!("Selected  {}", decorate(&original)),
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.pen.line_height + 6 * scale;
        self.pen.rule(hdc, pad, y, full - pad);
        y += 8 * scale;

        // --- body --------------------------------------------------------------------
        if self.rows.is_empty() {
            self.pen.draw(hdc, "No rulesets are loaded", pad, y, inner, text_colour, true);
            y += self.pen.line_height + 4 * scale;
        } else if self.expanded {
            y = self.paint_expanded(hdc, pad, y, inner, client, text_colour, dim);
        } else {
            y = self.paint_list(hdc, pad, y, inner, full, text_colour, dim);
        }

        // --- footer ------------------------------------------------------------------
        self.pen.rule(hdc, pad, y, full - pad);
        y += 8 * scale;
        if self.rows.is_empty() {
            self.pen.draw(hdc, "Esc close", pad, y, inner, dim, false);
            // SAFETY: restoring the object that was selected before this function ran.
            unsafe {
                SelectObject(hdc, previous);
            }
            return;
        }
        self.pen.draw(
            hdc,
            &format!(
                "\u{2191}\u{2193} {}   \u{2502}   <Enter> - apply   \u{2502}   <Space> - {}   \u{2502}   <ESC> - cancel",
                if self.expanded { "scroll" } else { "choose" },
                if self.expanded { "collapse" } else { "expand" }
            ),
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.pen.line_height;
        // The copy line, laid out exactly as the chooser's: the history toggle anchored to the
        // right edge so it cannot walk off the window when the prose beside it changes.
        let left = match self.copy_worked {
            None => "<Ctrl>+C / <Ctrl>+<Ins> - copy",
            Some(true) => "Copied \u{2713}",
            Some(false) => "Copy failed",
        };
        let gap = 5 * scale;
        let label = "keep the copy in clipboard history";
        let label_w = self.pen.measure(hdc, label);
        let hint_w = self.pen.measure(hdc, "<H>");
        let box_w =
            (self.pen.line_height - 8 * scale).clamp(11 * scale, 16 * scale);
        let group_x = pad + inner - (hint_w + gap + box_w + gap + label_w);

        self.pen
            .draw(hdc, left, pad, y, (group_x - pad - gap * 2).max(0), dim, false);
        self.pen.draw(hdc, "<H>", group_x, y, hint_w, dim, false);
        let drawn = self.pen.checkbox(hdc, group_x + hint_w + gap, y, self.expose_copy);
        self.pen.draw(
            hdc,
            label,
            group_x + hint_w + gap + drawn + gap,
            y,
            label_w,
            dim,
            false,
        );

        // SAFETY: restoring the object that was selected before this function ran.
        unsafe {
            SelectObject(hdc, previous);
        }
    }

    /// One row per ruleset: its name, a preview of its result, and on the right its languages
    /// and how many letters it left alone.
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
        let scale = self.pen.scale;
        let name_width = 220 * scale;
        let detail_width = 200 * scale;

        for (i, row) in self.rows.iter().enumerate() {
            let selected = i == self.selected;
            if selected {
                let band = RECT {
                    left: pad - 6 * scale,
                    top: y - 3 * scale,
                    right: full - pad + 6 * scale,
                    bottom: y + self.pen.line_height,
                };
                // SAFETY: `hdc` is valid for the paint and the brush is deleted immediately.
                unsafe {
                    let brush = CreateSolidBrush(COLORREF(GetSysColor(COLOR_HIGHLIGHT)));
                    FillRect(hdc, &band, brush);
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                }
            }
            let on_row = if selected {
                // SAFETY: no preconditions; returns a colour for a documented system index.
                COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHTTEXT) })
            } else if self.changes(row) {
                text_colour
            } else {
                // Dimmed, not dropped: the list never changes shape, so a ruleset stays where
                // the hand learned to find it.
                dim
            };
            let secondary = if selected { on_row } else { dim };

            self.pen
                .draw(hdc, &row.name, pad, y, name_width - 8 * scale, secondary, false);

            let middle = inner - name_width - detail_width;
            let room = middle / self.pen.char_width;
            let preview = compact(&row.text, self.original, room.max(8) as usize);
            self.pen.draw(
                hdc,
                &decorate(&preview),
                pad + name_width,
                y,
                middle,
                on_row,
                selected && self.changes(row),
            );

            // A result that looks complete but left letters alone is how half-converted text
            // gets pasted, so the count is shown rather than left to be noticed.
            let detail = if row.untouched > 0 {
                format!(
                    "{} left \u{00B7} {} \u{2192} {}",
                    row.untouched, row.source, row.target
                )
            } else if self.changes(row) {
                format!("{} \u{2192} {}", row.source, row.target)
            } else {
                "no change".to_string()
            };
            self.pen.draw_right(hdc, &detail, pad, y, inner, secondary);
            y += self.pen.line_height;
        }
        y + 4 * scale
    }

    /// The selected result in full, wrapped and scrollable. Drawn as the chooser draws its
    /// expanded view; see the comments there for why each step is the way it is.
    #[allow(clippy::too_many_arguments)]
    fn paint_expanded(
        &mut self,
        hdc: HDC,
        pad: i32,
        y: i32,
        inner: i32,
        client: RECT,
        text_colour: COLORREF,
        dim: COLORREF,
    ) -> i32 {
        let scale = self.pen.scale;
        let row = &self.rows[self.selected];
        let footer = self.pen.line_height * 2 + 8 * scale + 1.max(scale) + 14 * scale;
        let indicator_y = client.bottom - footer - self.pen.line_height;
        let bottom = indicator_y;

        self.pen.draw(
            hdc,
            &format!("{}   (Space closes, \u{2191}\u{2193} scrolls)", row.name),
            pad,
            y,
            inner,
            dim,
            false,
        );
        let top = y + self.pen.line_height;

        // `DrawTextW` faults on an empty slice from inside the window procedure.
        if row.text.is_empty() {
            return indicator_y + self.pen.line_height + 4 * scale;
        }

        let mut wide: Vec<u16> = row.text.encode_utf16().collect();
        let mut measured = RECT {
            left: pad,
            top,
            right: client.right - pad,
            bottom,
        };
        // SAFETY: `wide` and `measured` live for the whole call. DT_CALCRECT only measures, and
        // the buffer is rewritten only under DT_MODIFYSTRING, which is not passed.
        unsafe {
            DrawTextW(
                hdc,
                &mut wide,
                &mut measured,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
        }

        let visible = ((bottom - top) / self.pen.text_line).max(1);
        let total = ((measured.bottom - measured.top) / self.pen.text_line).max(1);
        let max_scroll = (total - visible).max(0) as usize;
        self.scroll = self.scroll.min(max_scroll);
        let clipped_bottom = top + visible * self.pen.text_line;

        let mut rect = RECT {
            left: pad,
            top: top - (self.scroll as i32 * self.pen.text_line),
            right: client.right - pad,
            bottom: clipped_bottom,
        };
        // SAFETY: `hdc` is valid for this paint; SaveDC's return is passed back to RestoreDC
        // unchanged, and `wide`/`rect` live across the DrawTextW call.
        unsafe {
            let saved = SaveDC(hdc);
            IntersectClipRect(hdc, pad, top, client.right - pad, clipped_bottom);
            SetTextColor(hdc, text_colour);
            DrawTextW(hdc, &mut wide, &mut rect, DT_WORDBREAK | DT_NOPREFIX);
            if saved != 0 {
                let _ = RestoreDC(hdc, saved);
            }
        }

        if max_scroll > 0 {
            self.pen.draw(
                hdc,
                &format!("line {} of {}", self.scroll + 1, max_scroll + 1),
                pad,
                indicator_y,
                inner,
                dim,
                false,
            );
        }
        indicator_y + self.pen.line_height + 4 * scale
    }
}

/// Render a preview with its omissions marked, on the side content was dropped from.
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
        // `lpCreateParams` is the pointer handed to CreateWindowExW — the `&mut Window` from
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
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Window;
    if ptr.is_null() {
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }
    // SAFETY: the pointer was stored by WM_NCCREATE from `show`'s stack frame, which does not
    // return until after DestroyWindow. Windows delivers messages for one window on one thread
    // at a time, so no other reference to the same `Window` is live during this call.
    let state = unsafe { &mut *ptr };

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: `ps` lives for the whole paint and EndPaint is called with the same
            // window and structure, as the API requires.
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                state.paint(hwnd, hdc);
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            // SAFETY: no preconditions; reports the current state of one virtual key.
            let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            if state.key(wp.0, ctrl) {
                if std::mem::take(&mut state.resized) {
                    let height = state.height_for(state.expanded);
                    // SAFETY: resizing a window this thread owns, which is alive for the
                    // duration of its own window procedure. A failure leaves the window at its
                    // previous size, which still paints.
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            WIDTH * state.pen.scale,
                            height,
                            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                // SAFETY: invalidating the whole client area of a window that is alive for the
                // duration of this call. `false` skips the background erase, which every paint
                // makes redundant and which flickers while scrolling.
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        // Clicking away is a cancellation; `Activation` decides what counts as clicking away.
        WM_ACTIVATE => {
            if state.decision.is_none() && state.activation.is_departure(hwnd, wp.0) {
                state.decision = Some(Decision::Cancel);
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
