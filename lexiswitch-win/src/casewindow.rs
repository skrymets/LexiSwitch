//! The window that asks which case the user meant.
//!
//! A sibling of [`chooser`](crate::chooser), not a mode of it
//! ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)). It keeps the
//! same promise — nothing is modified before an explicit commit — the same way: the selection
//! has already been read into a [`Transaction`](crate::transaction::Transaction), every result
//! has already been rendered from it, and this window does nothing but display them and return
//! a decision. It cannot modify the document because it is not given anything that could.
//!
//! It takes focus and runs on the worker thread for the reasons the chooser documents, and
//! shares the foreground handshake, fonts and drawing with it through
//! [`winui`](crate::winui) rather than copying them.
//!
//! ## The list never changes shape
//!
//! Every style is offered every time, in the declared order, whatever the text and whatever the
//! options. A style that would change nothing is drawn dim, not dropped. That is the opposite
//! of what the chooser does with layout candidates, and deliberately so: those are discovered
//! and unnamed, while these are fixed and each has a name the user aims at. A list that
//! reshuffled whenever a toggle was pressed would defeat the promoted entries above it.
//!
//! The same rule covers the toggles. One predicate — would flipping this change the result? —
//! decides whether a row or a toggle is drawn dim, so there is nothing to keep in step.

use std::ffi::c_void;

use lexiswitch_core::casing::{self, Row, Strips, Style, Toggle};
use lexiswitch_core::preview::compact;
use lexiswitch_core::usage::Usage;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, GetSysColor,
    IntersectClipRect, InvalidateRect, RestoreDC, SaveDC, SelectObject, SetBkMode, SetTextColor,
    COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
    COLOR_WINDOWTEXT,
    DT_CALCRECT, DT_NOPREFIX, DT_WORDBREAK, HDC, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
    CREATESTRUCTW, GWLP_USERDATA, IDC_ARROW, MSG, WM_ACTIVATE, WM_CLOSE, WM_KEYDOWN, WM_NCCREATE,
    WM_PAINT, WNDCLASSW, WS_BORDER, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::winui;
use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchCaseWindow");

/// How many styles are promoted to the top.
///
/// Three is chosen for the fixed height it gives the window rather than for any property of
/// three. Making it configurable is additive and can wait for evidence anyone wants a different
/// number.
const PROMOTED: usize = 3;

const VK_ESCAPE_U: usize = 0x1B;
const VK_RETURN_U: usize = 0x0D;
const VK_SPACE_U: usize = 0x20;
const VK_PRIOR_U: usize = 0x21;
const VK_NEXT_U: usize = 0x22;
const VK_UP_U: usize = 0x26;
const VK_DOWN_U: usize = 0x28;
const VK_HOME_U: usize = 0x24;
const VK_END_U: usize = 0x23;
const VK_R_U: usize = 0x52;
const VK_1_U: usize = 0x31;

/// How the user ended the transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Replace the selection with this style's result under these options.
    Apply { style: Style, strips: Strips },
    /// Change nothing.
    Cancel,
}

/// One line of the drawn list, which is not the same as one navigable row.
#[derive(Debug, Clone, Copy)]
enum Line {
    Heading(&'static str),
    Rule,
    Row(usize),
}

struct CaseWindow<'a> {
    original: &'a str,
    usage: &'a mut Usage,
    /// Every navigable row, promoted copies first.
    ///
    /// The promoted entries are a *copy*, not a move — the full list below keeps every style in
    /// its declared position, so promotion never takes a style out from under a hand that has
    /// learned where it is. A promoted row is otherwise indistinguishable from its twin, which
    /// is why this is a plain list of styles: the heading above them is the only difference,
    /// and committing either does exactly the same thing.
    entries: Vec<Style>,
    /// What is drawn, in order: headings, rules and rows.
    lines: Vec<Line>,
    selected: usize,
    strips: Strips,
    /// Every style's result under the current options, recomputed whenever a toggle changes.
    rendered: Vec<Row>,
    /// Whether the selected style's result is shown in full instead of the list.
    expanded: bool,
    /// First visible line of the expanded view.
    scroll: usize,
    /// First visible line of the list, used only when the window had to be clamped to the
    /// screen and the list no longer fits.
    top: usize,
    decision: Option<Decision>,
    activation: winui::Activation,
    pen: winui::Pen,
    expanded_lines: i32,
}

/// Open the case window and block until the user decides.
///
/// `usage` is borrowed mutably because the in-place reset has to take effect while the window is
/// open — a memory that has learned the wrong thing should be forgettable at the moment the user
/// notices, not by finding and deleting a file. The caller persists it afterwards.
///
/// Returns [`Decision::Cancel`] for every way out that is not an explicit commit, because the
/// transaction's default has to be "change nothing" for the promise above to mean anything.
pub fn show(
    original: &str,
    usage: &mut Usage,
    expanded_lines: i32,
) -> Result<Decision, WinError> {
    // SAFETY: the class name is a static wide string with a NUL and the WNDCLASSW is fully
    // initialised before registration. A zero return means the class is already registered by an
    // earlier invocation, which is harmless — CreateWindowExW below is the real check.
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
    // SAFETY: `class` is fully initialised and its name outlives the call.
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let strips = Strips::default();
    let mut state = CaseWindow {
        original,
        entries: Vec::new(),
        lines: Vec::new(),
        selected: 0,
        strips,
        rendered: casing::generate(original, &strips),
        expanded: false,
        scroll: 0,
        top: 0,
        decision: None,
        activation: winui::Activation::new(),
        pen: winui::Pen::new(winui::dpi_scale()),
        expanded_lines: if expanded_lines > 0 { expanded_lines } else { 10 },
        usage,
    };
    state.build();

    let (x, y, width, height) = state.geometry();
    let param = &mut state as *mut CaseWindow as *const c_void;

    // SAFETY: the class is registered, the strings are static and NUL-terminated, and `param`
    // points to a live `CaseWindow` that outlives this window — `state` is dropped only after
    // DestroyWindow below, and no other path leaves this function in between.
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

    if !state.activation.present(hwnd) {
        // SAFETY: `hwnd` was created above and has not been destroyed elsewhere. The fonts go
        // with `state` when this function returns.
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
    // `state` does, because the window procedure dereferences the pointer to it — and because
    // the fonts `state` owns must not be deleted while a paint could still select them.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    Ok(state.decision.clone().unwrap_or(Decision::Cancel))
}

impl CaseWindow<'_> {
    /// Work out the navigable rows and the drawn lines.
    ///
    /// Called once when the window opens and again only when the promoted set changes, which is
    /// the reset and nothing else. The promoted order is deliberately *not* recomputed as styles
    /// are applied: a list that reorders under the user's hand is worse than one that is a
    /// little stale.
    fn build(&mut self) {
        let promoted = self.usage.top(PROMOTED);
        self.entries.clear();
        self.lines.clear();

        if !promoted.is_empty() {
            self.lines.push(Line::Heading("Often used"));
            for style in &promoted {
                self.lines.push(Line::Row(self.entries.len()));
                self.entries.push(*style);
            }
            self.lines.push(Line::Rule);
        }

        for (i, group) in Style::GROUPS.iter().enumerate() {
            if i > 0 {
                self.lines.push(Line::Rule);
            }
            for style in *group {
                self.lines.push(Line::Row(self.entries.len()));
                self.entries.push(*style);
            }
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    /// The style currently under the cursor.
    fn style(&self) -> Style {
        self.entries[self.selected]
    }

    /// What that style would produce, and whether it is a change.
    fn row(&self) -> &Row {
        self.rendered
            .iter()
            .find(|r| r.style == self.style())
            .expect("every style is rendered")
    }

    fn result_for(&self, style: Style) -> &Row {
        self.rendered
            .iter()
            .find(|r| r.style == style)
            .expect("every style is rendered")
    }

    /// Where the window goes and how big it is.
    fn geometry(&self) -> (i32, i32, i32, i32) {
        let scale = self.pen.scale;
        let work = winui::work_area();
        let width = (820 * scale).min(work.right - work.left - 40 * scale);
        let height = self.wanted_height().min(work.bottom - work.top - 40 * scale);
        let (x, y) = winui::place_in(work, width, height);
        (x, y, width, height)
    }

    /// How tall the window would be with nothing cut off.
    ///
    /// Every term matches a step the paint takes, in the same order, so the two cannot drift.
    fn wanted_height(&self) -> i32 {
        let scale = self.pen.scale;
        let pad = 14 * scale;
        let line = self.pen.line_height;
        let body = if self.expanded {
            line + self.expanded_lines * self.pen.text_line + line
        } else {
            self.lines
                .iter()
                .map(|l| match l {
                    Line::Rule => self.rule_height(),
                    _ => line,
                })
                .sum()
        };
        pad + line * 2 + 6 * scale + self.rule_height() + body + self.rule_height()
            + line * 3
            + 4 * scale
            + line * 2
            + pad
    }

    fn rule_height(&self) -> i32 {
        8 * self.pen.scale + 1.max(self.pen.scale)
    }

    /// Recompute every result. Cheap, and the alternative is a cache that can be stale.
    fn refresh(&mut self) {
        self.rendered = casing::generate(self.original, &self.strips);
    }

    /// Apply a key press. Returns whether anything changed and the window needs repainting.
    fn key(&mut self, vk: usize, ctrl: bool) -> bool {
        // Discarding the counts, in place: the window stays open and the document untouched.
        if ctrl && vk == VK_R_U {
            self.usage.reset();
            self.build();
            return true;
        }
        // The toggle accelerators, which work without moving off the selected style. That is
        // the whole reason they are keys rather than rows in the list.
        if (VK_1_U..VK_1_U + Toggle::ALL.len()).contains(&vk) {
            self.strips.apply(Toggle::ALL[vk - VK_1_U]);
            self.refresh();
            return true;
        }

        match vk {
            VK_ESCAPE_U => {
                self.decision = Some(Decision::Cancel);
                false
            }
            VK_RETURN_U => {
                // Committing something that would change nothing cancels instead. A replacement
                // with identical text still costs a clipboard round trip through the target
                // application, which is the step most likely to go wrong, for no gain at all.
                self.decision = Some(if self.row().changes {
                    Decision::Apply {
                        style: self.style(),
                        strips: self.strips,
                    }
                } else {
                    Decision::Cancel
                });
                false
            }
            VK_SPACE_U => {
                self.expanded = !self.expanded;
                self.scroll = 0;
                true
            }
            // While expanded the arrows scroll the text; the selection is not moving, so the
            // style under review cannot change out from under the user mid-read.
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
            VK_UP_U => {
                self.selected = if self.selected == 0 {
                    self.entries.len() - 1
                } else {
                    self.selected - 1
                };
                true
            }
            VK_DOWN_U => {
                self.selected = (self.selected + 1) % self.entries.len();
                true
            }
            VK_PRIOR_U => {
                self.selected = self.selected.saturating_sub(5);
                true
            }
            VK_NEXT_U => {
                self.selected = (self.selected + 5).min(self.entries.len() - 1);
                true
            }
            VK_HOME_U => {
                self.selected = 0;
                true
            }
            VK_END_U => {
                self.selected = self.entries.len() - 1;
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
        // Every object selected here is restored or is a stock value.
        unsafe {
            let bg = CreateSolidBrush(COLORREF(GetSysColor(COLOR_WINDOW)));
            FillRect(hdc, &client, bg);
            let _ = DeleteObject(HGDIOBJ(bg.0));
            SetBkMode(hdc, TRANSPARENT);
        }
        // SAFETY: as above; the previous font is restored at the end of this function.
        let previous = unsafe { SelectObject(hdc, HGDIOBJ(self.pen.face().0)) };

        // SAFETY: no preconditions; each returns a colour for a documented system index.
        let text_colour = COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) });
        // SAFETY: as above.
        let dim = COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) });

        let full = client.right - client.left;
        let inner = full - pad * 2;
        let mut y = pad;

        // --- header ------------------------------------------------------------------
        self.pen
            .draw(hdc, "Change case", pad, y, inner, text_colour, true);
        y += self.pen.line_height;

        let chars = (inner / self.pen.char_width).max(8) as usize;
        let shown = compact(self.original, self.original, chars.saturating_sub(12));
        self.pen.draw(
            hdc,
            &format!("Selected  {}", decorate(&shown)),
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.pen.line_height + 6 * scale;
        self.pen.rule(hdc, pad, y, full - pad);
        y += self.rule_height();

        // --- body --------------------------------------------------------------------
        let footer_height = self.rule_height() + self.pen.line_height * 5 + 4 * scale + pad;
        let body_bottom = client.bottom - footer_height;
        if self.expanded {
            self.paint_expanded(hdc, pad, y, inner, client, body_bottom, text_colour, dim);
        } else {
            self.paint_list(hdc, pad, y, inner, full, body_bottom, text_colour, dim);
        }

        // --- toggles and footer ------------------------------------------------------
        let mut y = client.bottom - footer_height;
        self.pen.rule(hdc, pad, y, full - pad);
        y += self.rule_height();
        y = self.paint_toggles(hdc, pad, y, inner, text_colour, dim);
        y += 4 * scale;
        self.pen.draw(
            hdc,
            "\u{2191}\u{2193} choose    1-6 strip    Space expand    Ctrl+R reset order",
            pad,
            y,
            inner,
            dim,
            false,
        );
        y += self.pen.line_height;
        self.pen.draw(
            hdc,
            if self.row().changes {
                "Enter apply    Esc cancel"
            } else {
                "Enter closes without changing anything    Esc cancel"
            },
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

    /// The list of styles, with headings and group rules.
    #[allow(clippy::too_many_arguments)]
    fn paint_list(
        &mut self,
        hdc: HDC,
        pad: i32,
        top: i32,
        inner: i32,
        full: i32,
        bottom: i32,
        text_colour: COLORREF,
        dim: COLORREF,
    ) {
        let scale = self.pen.scale;
        let label_width = 210 * scale;

        // The window is sized to hold the whole list, and normally does. It is clamped to the
        // work area, though, so at a large display scale the list can be taller than the screen
        // — and a clamped window that silently cuts off the last style is worse than one that
        // scrolls. This keeps the selected row on screen and is otherwise inert.
        let room = ((bottom - top) / self.pen.line_height).max(1) as usize;
        let selected_line = self
            .lines
            .iter()
            .position(|l| matches!(l, Line::Row(i) if *i == self.selected))
            .unwrap_or(0);
        if selected_line < self.top {
            self.top = selected_line;
        } else if selected_line >= self.top + room {
            self.top = selected_line + 1 - room;
        }
        self.top = self.top.min(self.lines.len().saturating_sub(room));

        // Say so when the list does not all fit. Without this the only evidence that there is
        // more below is that pressing Down eventually reveals it, which is a thing to discover
        // rather than a thing to see — and at a large display scale, where the window is clamped
        // to the work area, not fitting is the normal case rather than the odd one.
        let hidden = self.lines.len() > room;
        if hidden {
            let scale = self.pen.scale;
            let track = RECT {
                left: full - pad + 2 * scale,
                top,
                right: full - pad + 5 * scale,
                bottom,
            };
            let span = (bottom - top).max(1);
            let visible = (span * room as i32 / self.lines.len() as i32).max(8 * scale);
            let offset = (span - visible) * self.top as i32
                / (self.lines.len() - room).max(1) as i32;
            let thumb = RECT {
                top: top + offset,
                bottom: top + offset + visible,
                ..track
            };
            // SAFETY: `hdc` is valid for this paint and both brushes are deleted immediately.
            unsafe {
                let back = CreateSolidBrush(COLORREF(GetSysColor(COLOR_BTNFACE)));
                FillRect(hdc, &track, back);
                let _ = DeleteObject(HGDIOBJ(back.0));
                let front = CreateSolidBrush(COLORREF(GetSysColor(COLOR_GRAYTEXT)));
                FillRect(hdc, &thumb, front);
                let _ = DeleteObject(HGDIOBJ(front.0));
            }
        }

        let mut y = top;
        for line in self.lines.iter().skip(self.top) {
            match line {
                Line::Rule => {
                    if y + self.rule_height() > bottom {
                        break;
                    }
                    self.pen.rule(hdc, pad, y + 3 * scale, full - pad);
                    y += self.rule_height();
                }
                Line::Heading(text) => {
                    if y + self.pen.line_height > bottom {
                        break;
                    }
                    self.pen.draw(hdc, text, pad, y, inner, dim, true);
                    y += self.pen.line_height;
                }
                Line::Row(index) => {
                    if y + self.pen.line_height > bottom {
                        break;
                    }
                    let style = self.entries[*index];
                    let row = self.result_for(style);
                    let selected = *index == self.selected;

                    if selected {
                        let box_ = RECT {
                            left: pad - 6 * scale,
                            top: y - 2 * scale,
                            right: full - pad + 6 * scale,
                            bottom: y + self.pen.line_height,
                        };
                        // SAFETY: `hdc` is valid for the paint; the brush is deleted at once.
                        unsafe {
                            let brush = CreateSolidBrush(COLORREF(GetSysColor(COLOR_HIGHLIGHT)));
                            FillRect(hdc, &box_, brush);
                            let _ = DeleteObject(HGDIOBJ(brush.0));
                        }
                    }

                    // Dim means "this would change nothing". The row is still here and still
                    // selectable — removing it is what would move everything else.
                    let colour = if selected {
                        // SAFETY: no preconditions.
                        COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHTTEXT) })
                    } else if row.changes {
                        text_colour
                    } else {
                        dim
                    };
                    let label_colour = if selected { colour } else { dim };

                    self.pen.draw(
                        hdc,
                        style.label(),
                        pad,
                        y,
                        label_width,
                        label_colour,
                        false,
                    );
                    let room = ((inner - label_width) / self.pen.char_width).max(8) as usize;
                    let preview = compact(&row.text, self.original, room);
                    self.pen.draw(
                        hdc,
                        &decorate(&preview),
                        pad + label_width,
                        y,
                        inner - label_width,
                        colour,
                        selected && row.changes,
                    );
                    y += self.pen.line_height;
                }
            }
        }
    }

    /// The six toggles on three labelled rows, so the two exclusive pairs read as pairs.
    fn paint_toggles(
        &self,
        hdc: HDC,
        pad: i32,
        top: i32,
        inner: i32,
        text_colour: COLORREF,
        dim: COLORREF,
    ) -> i32 {
        let scale = self.pen.scale;
        let heading_width = 70 * scale;
        let column = (inner - heading_width) / 2;
        let style = self.style();

        // Grouped exactly as they behave: the two pairs share a row each, so the exclusivity is
        // legible before anything is pressed rather than discovered when a box the user did not
        // touch clears itself.
        let rows: [(&str, [Toggle; 2]); 3] = [
            ("Strip", [Toggle::Punctuation, Toggle::Digits]),
            ("Spaces", [Toggle::RemoveSpaces, Toggle::NormalizeSpaces]),
            ("Lines", [Toggle::JoinLines, Toggle::RemoveLines]),
        ];

        let mut y = top;
        for (heading, toggles) in rows {
            self.pen.draw(hdc, heading, pad, y, heading_width, dim, false);
            for (i, toggle) in toggles.iter().enumerate() {
                let number = Toggle::ALL
                    .iter()
                    .position(|t| t == toggle)
                    .map(|n| n + 1)
                    .unwrap_or(0);
                let mark = if self.strips.is_set(*toggle) { "x" } else { " " };
                // Same rule as the rows: dim when flipping it would not change this result.
                let colour = if casing::toggle_affects(self.original, style, &self.strips, *toggle)
                {
                    text_colour
                } else {
                    dim
                };
                self.pen.draw(
                    hdc,
                    &format!("{number} [{mark}] {}", toggle.label()),
                    pad + heading_width + column * i as i32,
                    y,
                    column,
                    colour,
                    false,
                );
            }
            y += self.pen.line_height;
        }
        y
    }

    /// The selected style's result in full, wrapped and scrollable.
    #[allow(clippy::too_many_arguments)]
    fn paint_expanded(
        &mut self,
        hdc: HDC,
        pad: i32,
        top: i32,
        inner: i32,
        client: RECT,
        bottom: i32,
        text_colour: COLORREF,
        dim: COLORREF,
    ) {
        let scale = self.pen.scale;
        let indicator_y = bottom - self.pen.line_height;
        let style = self.style();
        let text = self.result_for(style).text.clone();

        self.pen.draw(
            hdc,
            &format!(
                "{}   (Space closes, \u{2191}\u{2193} scrolls)",
                style.label()
            ),
            pad,
            top,
            inner,
            dim,
            false,
        );
        let text_top = top + self.pen.line_height;

        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut measured = RECT {
            left: pad,
            top: text_top,
            right: client.right - pad,
            bottom: indicator_y,
        };
        // SAFETY: `wide` and `measured` live for the whole call. DT_CALCRECT only measures — it
        // draws nothing — and the buffer is rewritten only under DT_MODIFYSTRING, not passed.
        unsafe {
            DrawTextW(
                hdc,
                &mut wide,
                &mut measured,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
        }

        let visible = ((indicator_y - text_top) / self.pen.text_line).max(1);
        let total = ((measured.bottom - measured.top) / self.pen.text_line).max(1);
        let max_scroll = (total - visible).max(0) as usize;
        self.scroll = self.scroll.min(max_scroll);

        // Clipped to a whole number of lines. Stopping partway through one leaves a sliced row
        // of half-glyphs, which reads as a drawing fault rather than as more text below.
        let clipped_bottom = text_top + visible * self.pen.text_line;
        let mut rect = RECT {
            left: pad,
            top: text_top - (self.scroll as i32 * self.pen.text_line),
            right: client.right - pad,
            bottom: clipped_bottom,
        };
        // Clipped explicitly, because DrawTextW honours the *bottom* of its rectangle and
        // happily draws above the top — and scrolling works by moving the top upward.
        //
        // SAFETY: `hdc` is valid for this paint; SaveDC's return goes back to RestoreDC
        // unchanged, and `wide`/`rect` live across the DrawTextW call.
        unsafe {
            let saved = SaveDC(hdc);
            IntersectClipRect(hdc, pad, text_top, client.right - pad, clipped_bottom);
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
        let _ = scale;
    }
}

/// Render a preview with its omissions marked.
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
/// window of this class. `lp` is a `CREATESTRUCTW` for `WM_NCCREATE` and is not dereferenced for
/// any other message.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: Windows documents `lp` as a pointer to a CREATESTRUCTW for this message, and
        // `lpCreateParams` is the pointer handed to CreateWindowExW — the `&mut CaseWindow` from
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
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut CaseWindow;
    if ptr.is_null() {
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }
    // SAFETY: the pointer was stored by WM_NCCREATE from `show`'s stack frame, which does not
    // return until after DestroyWindow. Windows delivers messages for one window on one thread
    // at a time, so no other reference to the same `CaseWindow` is live during this call.
    let state = unsafe { &mut *ptr };

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: `ps` lives for the whole paint and EndPaint is called with the same window
            // and structure, as the API requires.
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
                // SAFETY: invalidating the whole client area of a window that is alive for the
                // duration of this call.
                unsafe {
                // `false`: do not let Windows erase the background first. Every paint here
                // fills the whole client area before drawing anything, so the erase adds a
                // frame of blank window and nothing else — which is the flicker seen while
                // scrolling, where repaints come in quick succession.
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        // Clicking away is a cancellation. What counts as clicking away is `Activation`'s to
        // decide: the transients of the foreground handoff are not it.
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
