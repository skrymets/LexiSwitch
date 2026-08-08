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

const CLASS_NAME: PCWSTR = w!("LexiSwitchChooser");

/// How many wrapped lines the expanded view shows before it has to scroll, when the caller
/// expresses no preference.
const EXPANDED_LINES: i32 = 10;

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
    /// Set when a toggle changed how tall the window needs to be, so the window procedure can
    /// resize it before repainting.
    resized: bool,
    /// First visible line of the expanded view.
    scroll: usize,
    decision: Option<Decision>,
    /// Whether losing activation should be treated as the user clicking away. See
    /// [`winui::Activation`], which exists because this rule was got wrong once it was copied.
    activation: winui::Activation,
    /// Fonts, measurements and drawing.
    pen: winui::Pen,
    /// How many wrapped lines the expanded view shows, from configuration.
    expanded_lines: i32,
}

/// Open the chooser and block until the user decides.
///
/// Returns [`Decision::Cancel`] for every way out that is not an explicit commit — Escape, the
/// window closing, or focus moving elsewhere — because the transaction's default has to be
/// "change nothing" for the promise above to mean anything.
///
/// An empty `candidates` is a state, not an error: the chooser opens and says that no distinct
/// conversion was found. The spec requires feedback about a transaction to appear *in* the
/// transaction, and requires that outcome to be distinguishable from a failure — a balloon
/// after the window declined to open is neither.
pub fn show(
    original: &str,
    candidates: &[Candidate],
    active: Option<&str>,
    expanded_lines: i32,
) -> Result<Decision, WinError> {

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

    let mut state = Chooser {
        original,
        candidates,
        active,
        selected: 0,
        expanded: false,
        resized: false,
        scroll: 0,
        decision: None,
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

    // A chooser that is not in front receives no keys, and its modal loop has no way out —
    // Escape would go to whatever kept the foreground. Refusing is the only outcome that is
    // not a hang, and it leaves the transaction to be cancelled cleanly by the caller.
    if !state.activation.present(hwnd) {
        // SAFETY: `hwnd` was created above and has not been destroyed elsewhere. The fonts go
        // with `state` when this function returns.
        unsafe {
            let _ = DestroyWindow(hwnd);
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
    // `state` does, because the window procedure dereferences the pointer to it — and because
    // the fonts `state` owns must not be deleted while a paint could still select them.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    Ok(state.decision.unwrap_or(Decision::Cancel))
}


/// Where the window goes and how big it is.
///
/// Centred on the monitor holding the foreground window, per design.md — near-caret placement
/// is deliberately deferred, because it needs caret geometry, edge repositioning and an overlay
/// that would cover the very text under review.
fn geometry(state: &Chooser) -> (i32, i32, i32, i32) {
    let scale = state.pen.scale;
    let work = winui::work_area();
    // Never wider or taller than the screen it has to fit on.
    let width = (760 * scale).min(work.right - work.left - 40 * scale);
    let height = state
        .height_for(state.expanded)
        .min(work.bottom - work.top - 40 * scale);
    let (x, y) = winui::place_in(work, width, height);
    (x, y, width, height)
}

impl Chooser<'_> {
    /// How tall the window has to be in the given mode.
    ///
    /// Every term matches a step the paint takes, in the same order, so the two cannot drift.
    /// This is what stopped the footer being clipped: the layout advances by measured text
    /// metrics, so the height must be built from the same ones rather than from round numbers.
    fn height_for(&self, expanded: bool) -> i32 {
        let scale = self.pen.scale;
        let pad = 14 * scale;
        let line = self.pen.line_height;
        let body = if expanded {
            // The candidate's own heading, room for a useful number of wrapped lines, and the
            // scroll position. Without this the window kept its list height and "expanded"
            // showed a single line, which is not an expansion of anything.
            line + self.expanded_lines * self.pen.text_line + line
        } else {
            // One line either way: with no candidates that line carries the explanation.
            (self.candidates.len() as i32).max(2) * line
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

    /// Apply a key press. Returns whether anything changed and the window needs repainting.
    fn key(&mut self, vk: usize, ctrl: bool) -> bool {
        if self.candidates.is_empty() {
            // Nothing to move between and nothing to apply. Any key that means "done" closes
            // it; everything else is ignored rather than silently doing something else.
            if matches!(vk, VK_ESCAPE_U | VK_RETURN_U | VK_SPACE_U) {
                self.decision = Some(Decision::Cancel);
            }
            return false;
        }
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
                self.resized = true;
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
    fn paint(&mut self, hwnd: HWND, hdc: HDC) {
        // The client rect, never the damaged rectangle. `BeginPaint` reports only the
        // region that needs repainting, and laying the window out against that would
        // place everything relative to whatever happened to be invalidated.
        let mut client = RECT::default();
        // SAFETY: `client` lives for the whole call and `hwnd` is alive while its own
        // window procedure runs.
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
        self.pen.draw(hdc, "Convert selection", pad, y, inner, text_colour, true);
        if let Some(active) = self.active {
            // Labelled "Typing in", never "from": the active layout is context, and presenting
            // it as any candidate's source is precisely the confusion the spec forbids.
            self.pen.draw_right(
                hdc,
                &format!("Typing in {active}"),
                pad,
                y,
                inner,
                dim,
            );
        }
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
        if self.candidates.is_empty() {
            y = self.paint_nothing(hdc, pad, y, inner, text_colour, dim);
        } else if self.expanded {
            y = self.paint_expanded(hdc, pad, y, inner, client, text_colour, dim);
        } else {
            y = self.paint_list(hdc, pad, y, inner, full, text_colour, dim);
        }

        // --- footer ------------------------------------------------------------------
        self.pen.rule(hdc, pad, y, full - pad);
        y += 8 * scale;
        if self.candidates.is_empty() {
            self.pen.draw(hdc, "Esc close", pad, y, inner, dim, false);
            // SAFETY: restoring the object that was selected before this function ran.
            unsafe {
                SelectObject(hdc, previous);
            }
            return;
        }
        let target = &self.candidates[self.selected].to;
        self.pen.draw(
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
        y += self.pen.line_height;
        // Task 3.7: the layout a switching commit would activate is named before the user
        // commits, not discovered afterwards.
        self.pen.draw(
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

    /// The state where the selection was read perfectly well and no conversion changes it.
    ///
    /// Deliberately worded as a result rather than a problem. Nothing failed here: every
    /// installed layout was tried and each produced the text that is already there, which is
    /// most often what happens when the text was never mistyped in the first place.
    #[allow(clippy::too_many_arguments)]
    fn paint_nothing(
        &self,
        hdc: HDC,
        pad: i32,
        mut y: i32,
        inner: i32,
        text_colour: COLORREF,
        dim: COLORREF,
    ) -> i32 {
        self.pen.draw(hdc, "No conversion found", pad, y, inner, text_colour, true);
        y += self.pen.line_height;
        self.pen.draw(
            hdc,
            "Every installed layout produces the text that is already there. Nothing was \
             changed.",
            pad,
            y,
            inner,
            dim,
            false,
        );
        y + self.pen.line_height + 4 * self.pen.scale
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
        let scale = self.pen.scale;
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
                    bottom: y + self.pen.line_height,
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

            self.pen.draw(
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
            let room = (inner - label_width - count_width) / self.pen.char_width;
            let preview = compact(&candidate.text, self.original, (room.max(8)) as usize);
            self.pen.draw(
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
                self.pen.draw_right(
                    hdc,
                    &format!("{} unchanged", candidate.unchanged),
                    pad,
                    y,
                    inner,
                    secondary,
                );
            }
            y += self.pen.line_height;
        }
        y + 4 * scale
    }

    /// The selected candidate in full, wrapped and scrollable.
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
        let candidate = &self.candidates[self.selected];
        // Where the body has to stop: the footer's two lines, its rule, and the padding under
        // it, plus one line kept back for the scroll position. That line is reserved whether or
        // not there is anything to scroll, so the text does not shift under the reader the
        // moment the indicator appears.
        let footer = self.pen.line_height * 2 + 8 * scale + 1.max(scale) + 14 * scale;
        let indicator_y = client.bottom - footer - self.pen.line_height;
        let bottom = indicator_y;

        self.pen.draw(
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
        let top = y + self.pen.line_height;

        let mut wide: Vec<u16> = candidate.text.encode_utf16().collect();

        // How tall the wrapped text is, asked of the same function that will draw it so the
        // two cannot disagree about where the lines break.
        let mut measured = RECT {
            left: pad,
            top,
            right: client.right - pad,
            bottom,
        };
        // SAFETY: `wide` and `measured` live for the whole call. DT_CALCRECT only measures —
        // it draws nothing — and the buffer is rewritten only under DT_MODIFYSTRING, which is
        // not passed.
        unsafe {
            DrawTextW(
                hdc,
                &mut wide,
                &mut measured,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
        }

        // Clamp the scroll so it cannot run off the end into blank space. One line is always
        // left visible, so scrolling never empties the view.
        let visible = ((bottom - top) / self.pen.text_line).max(1);
        let total = ((measured.bottom - measured.top) / self.pen.text_line).max(1);
        let max_scroll = (total - visible).max(0) as usize;
        self.scroll = self.scroll.min(max_scroll);

        // Clip to a whole number of lines. Stopping partway through one leaves a sliced row of
        // half-glyphs, which looks like a drawing fault rather than like more text below.
        let clipped_bottom = top + visible * self.pen.text_line;

        let mut rect = RECT {
            left: pad,
            top: top - (self.scroll as i32 * self.pen.text_line),
            right: client.right - pad,
            bottom: clipped_bottom,
        };

        // Clipped to the body, because DrawTextW does not do it. It honours the *bottom* of
        // its rectangle but happily draws above the top — and scrolling works by moving the
        // top upward, so the text was being painted straight over the header line above it.
        // The DC state is saved and restored rather than the clip merely being widened again,
        // since IntersectClipRect only ever narrows.
        //
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

        // Say so when there is more text than fits, rather than leaving the user to discover
        // it by pressing an arrow and watching nothing happen.
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
                    // Grown or shrunk to fit the mode just switched into. The top-left corner
                    // stays put: re-centring on every toggle would move the window out from
                    // under the reader's eye for no gain.
                    let height = state.height_for(state.expanded);
                    // SAFETY: resizing a window this thread owns, which is alive for the
                    // duration of its own window procedure. A failure is reported as a bool
                    // and leaves the window at its previous size, which still paints.
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            760 * state.pen.scale,
                            height,
                            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
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
        // Clicking away is a cancellation, not an invitation to leave a chooser stranded on
        // screen holding a transaction open. What counts as clicking away is `Activation`'s to
        // decide: the transients of the foreground handoff are not it.
        WM_ACTIVATE => {
            #[cfg(feature = "chooser-diag")]
            eprintln!(
                "[chooser] WM_ACTIVATE state=0x{:04X} other=0x{:X}",
                wp.0 & 0xFFFF,
                lp.0
            );
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
