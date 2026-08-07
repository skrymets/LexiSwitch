//! The window that records a replacement shortcut.
//!
//! Reached when the configured combination is held by another application, which used to end
//! the process. The point of keeping it running is that the user can fix it — and fixing it has
//! to be possible from the keyboard, without a restart and without editing a file.
//!
//! ## How it reads keys, and how it must not
//!
//! As a focused window, through `WM_KEYDOWN`. That is the whole mechanism.
//!
//! The tempting alternative is to observe combinations globally so the user can press one
//! without the window taking focus. That is a low-level keyboard hook, which
//! [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md) forbids outright — the same
//! constraint that shapes the chooser, and for the same reason. A shortcut recorder is exactly
//! the feature that would justify a hook if anything did, so it is worth being explicit: this
//! one does not have one.
//!
//! ## Escape and Enter
//!
//! Both are keys a user might legitimately want to bind, so they cannot simply be reserved.
//! They mean cancel and accept **only when no modifier is held**, which leaves `Ctrl+Enter`,
//! `Shift+Escape` and every other modified form recordable. A bare `Enter` as a global shortcut
//! would be unusable anyway.

use lexiswitch_core::state::{suggested_shortcuts, Shortcut};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetSysColor, InvalidateRect,
    SelectObject, SetBkMode, COLOR_GRAYTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, DT_CENTER, DT_LEFT,
    HDC, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LWIN,
    VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
    CREATESTRUCTW, GWLP_USERDATA, IDC_ARROW, MSG, WM_ACTIVATE, WM_CLOSE, WM_KEYDOWN, WM_NCCREATE,
    WM_PAINT, WM_SYSKEYDOWN, WNDCLASSW, WS_BORDER, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::hotkey::format_binding;
use crate::winui;
use crate::WinError;

const CLASS_NAME: PCWSTR = w!("LexiSwitchRecorder");

const VK_ESCAPE_U: usize = 0x1B;
const VK_RETURN_U: usize = 0x0D;

/// What the user did with the recorder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    /// A combination to try. Rendered in the form the configuration file uses.
    Chosen(String),
    /// Closed without choosing. The existing shortcut, whatever its state, is untouched.
    Cancelled,
}

struct Recorder<'a> {
    /// The shortcut as it stands, shown so the user can see what they are replacing.
    current: &'a Shortcut,
    /// Why the last attempt was refused, if there was one.
    rejected: Option<String>,
    /// The combination captured so far, ready to be accepted.
    candidate: Option<String>,
    done: Option<Recorded>,
    /// Whether losing activation should be treated as the user clicking away.
    ///
    /// This window once reproduced the chooser's activation bug in full — cancelling itself
    /// instantly and logging `shortcut_recording_cancelled` with nothing on screen — because
    /// the fix lived in the chooser's own state rather than anywhere shared. It now lives in
    /// [`winui::Activation`], which is the shared place.
    activation: winui::Activation,
    pen: winui::Pen,
}

/// Open the recorder and block until the user chooses a combination or gives up.
///
/// `rejected` is shown when this is a second attempt — a combination that could not be
/// registered is reported here rather than in a balloon the user has to go and find, and the
/// previous shortcut is left alone until a replacement actually works.
pub fn show(current: &Shortcut, rejected: Option<String>) -> Result<Recorded, WinError> {
    // SAFETY: the class name is static with a NUL and the WNDCLASSW is fully initialised. A
    // zero return means the class was registered by an earlier invocation, which is harmless.
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

    let mut state = Recorder {
        current,
        rejected,
        candidate: None,
        done: None,
        activation: winui::Activation::new(),
        pen: winui::Pen::new(winui::dpi_scale()),
    };

    let scale = state.pen.scale;
    let width = 620 * scale;
    let height = state.pen.line_height * 8 + 28 * scale;
    let (x, y) = winui::centre_on_active_monitor(width, height);

    let param = &mut state as *mut Recorder as *const std::ffi::c_void;
    // SAFETY: the class is registered, the strings are static, and `param` points to a live
    // `Recorder` that outlives the window — it is dropped only after DestroyWindow below.
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
        // SAFETY: `hwnd` was just created here and is still alive. The font goes with `state`
        // when this function returns.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return Err(WinError::last("SetForegroundWindow"));
    }

    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is valid for the whole call; 0 and -1 both end the loop.
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        // SAFETY: `msg` was filled by GetMessageW and is untouched since.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if state.done.is_some() {
            break;
        }
    }

    // SAFETY: created here, not destroyed elsewhere, and destroyed before `state` is dropped —
    // which is what stops the font going while a paint could still select it.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    Ok(state.done.unwrap_or(Recorded::Cancelled))
}

/// The modifiers currently held, in the form `RegisterHotKey` wants.
fn held_modifiers() -> HOT_KEY_MODIFIERS {
    let mut m = HOT_KEY_MODIFIERS(0);
    // SAFETY: no preconditions; each call reports one key's state. The high bit is "down",
    // which is the sign bit of the i16 returned.
    unsafe {
        if GetKeyState(VK_CONTROL.0 as i32) < 0 {
            m |= MOD_CONTROL;
        }
        if GetKeyState(VK_MENU.0 as i32) < 0 {
            m |= MOD_ALT;
        }
        if GetKeyState(VK_SHIFT.0 as i32) < 0 {
            m |= MOD_SHIFT;
        }
        if GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0 {
            m |= MOD_WIN;
        }
    }
    m
}

/// Whether a virtual key is itself a modifier rather than a key to bind.
fn is_modifier(vk: usize) -> bool {
    matches!(
        vk,
        0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
    )
}

impl Recorder<'_> {
    /// Apply a key press. Returns whether the window needs repainting.
    fn key(&mut self, vk: usize) -> bool {
        // A modifier on its own is not a combination; it is the user still assembling one.
        if is_modifier(vk) {
            return false;
        }
        let modifiers = held_modifiers();
        let bare = modifiers == HOT_KEY_MODIFIERS(0);

        // Escape and Enter mean cancel and accept only unmodified, so every modified form of
        // them remains recordable.
        if bare && vk == VK_ESCAPE_U {
            self.done = Some(Recorded::Cancelled);
            return false;
        }
        if bare && vk == VK_RETURN_U {
            if let Some(candidate) = self.candidate.clone() {
                self.done = Some(Recorded::Chosen(candidate));
            }
            return false;
        }

        match format_binding(modifiers, vk as u32) {
            Some(text) => {
                self.candidate = Some(text);
                self.rejected = None;
            }
            // A key with no name cannot be written to the configuration file, so it is refused
            // here rather than persisted as something the loader would reject.
            None => {
                self.rejected = Some("That key cannot be used in a shortcut.".to_string());
                self.candidate = None;
            }
        }
        true
    }

    fn paint(&self, hdc: HDC, client: RECT) {
        let scale = self.pen.scale;
        let pad = 18 * scale;
        let inner = client.right - client.left - pad * 2;

        // SAFETY: `hdc` is the paint DC and every object created here is deleted immediately.
        unsafe {
            let bg = CreateSolidBrush(COLORREF(GetSysColor(COLOR_WINDOW)));
            FillRect(hdc, &client, bg);
            let _ = DeleteObject(HGDIOBJ(bg.0));
            SetBkMode(hdc, TRANSPARENT);
        }
        // SAFETY: no preconditions.
        let text_colour = COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) });
        // SAFETY: as above.
        let dim = COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) });

        let mut y = pad;
        self.pen.draw_aligned(hdc, "Choose a shortcut", pad, y, inner, text_colour, DT_LEFT);
        y += self.pen.line_height;

        let current = match self.current {
            Shortcut::Registered(b) => format!("Currently {b}"),
            Shortcut::Conflict(b) => format!("{b} is held by another application"),
        };
        self.pen.draw_aligned(hdc, &current, pad, y, inner, dim, DT_LEFT);
        y += self.pen.line_height + 8 * scale;

        let shown = self
            .candidate
            .clone()
            .unwrap_or_else(|| "Press a combination…".to_string());
        self.pen.draw_aligned(hdc, &shown, pad, y, inner, text_colour, DT_CENTER);
        y += self.pen.line_height + 8 * scale;

        if let Some(why) = &self.rejected {
            self.pen.draw_aligned(hdc, why, pad, y, inner, text_colour, DT_LEFT);
        }
        y += self.pen.line_height;

        self.pen.draw_aligned(
            hdc,
            "Hold modifiers and press a key. Enter accepts, Escape cancels.",
            pad,
            y,
            inner,
            dim,
            DT_LEFT,
        );
        y += self.pen.line_height;
        self.pen.draw_aligned(
            hdc,
            "Enter and Escape can themselves be recorded when held with a modifier.",
            pad,
            y,
            inner,
            dim,
            DT_LEFT,
        );
        y += self.pen.line_height;
        // Suggestions, never promises: whether any of these is free depends on every other
        // application running, and the only way to find out is to try one.
        let current = match self.current {
            Shortcut::Registered(b) | Shortcut::Conflict(b) => b.as_str(),
        };
        let ideas = suggested_shortcuts(current);
        if !ideas.is_empty() {
            self.pen.draw_aligned(
                hdc,
                &format!("Often free, but not guaranteed: {}", ideas.join(", ")),
                pad,
                y,
                inner,
                dim,
                DT_LEFT,
            );
        }
    }

}

/// SAFETY (contract on the caller, which is Windows): invoked by the window manager for a
/// window of this class. `lp` is a `CREATESTRUCTW` for `WM_NCCREATE` and is not dereferenced
/// for any other message.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: Windows documents `lp` as a CREATESTRUCTW for this message, and
        // `lpCreateParams` is the pointer handed to CreateWindowExW.
        let state = unsafe { (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams };
        // SAFETY: storing an integer in this window's user data; no dereference here.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        }
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }

    // SAFETY: reading back the integer stored above; zero until WM_NCCREATE has run.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Recorder;
    if ptr.is_null() {
        // SAFETY: forwarding the exact arguments Windows passed.
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    }
    // SAFETY: the pointer was stored from `show`'s stack frame, which does not return until
    // after DestroyWindow. Windows delivers one window's messages on one thread at a time.
    let state = unsafe { &mut *ptr };

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: `ps` lives for the whole paint and EndPaint gets the same window.
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut client = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client);
                let previous = SelectObject(hdc, HGDIOBJ(state.pen.face().0));
                state.paint(hdc, client);
                SelectObject(hdc, previous);
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        // WM_SYSKEYDOWN as well: Alt combinations arrive as system keys, and a recorder that
        // ignored them could not record anything containing Alt.
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if state.key(wp.0) {
                // SAFETY: invalidating a window that is alive for this call.
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
            LRESULT(0)
        }
    
        // Clicking away abandons the recording rather than leaving a window listening to a
        // keyboard aimed at something else — but only once it is really in front, and only if
        // it has genuinely lost the foreground rather than merely been told so.
        WM_ACTIVATE => {
            if state.done.is_none() && state.activation.is_departure(hwnd, wp.0) {
                state.done = Some(Recorded::Cancelled);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            state.done = Some(Recorded::Cancelled);
            LRESULT(0)
        }
        // SAFETY: forwarding the exact arguments Windows passed.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}
