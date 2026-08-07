//! The tray icon and its notifications.
//!
//! There is no console and no log by default, so a failure that does not reach the tray is
//! invisible. The `agent-control` spec requires notifications to state what did *not*
//! happen — "clipboard busy, selection not converted", never "error 5".
//!
//! The menu needs the window that owns the icon to be the foreground window, which is why
//! that window is an ordinary hidden top-level one rather than message-only — see
//! [`crate::window`].

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFY_ICON_INFOTIP_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, PostMessageW,
    SetForegroundWindow, TrackPopupMenu, HICON, IDI_APPLICATION, IDI_HAND, IDI_WARNING, MF_GRAYED, MF_SEPARATOR,
    MF_STRING, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_NULL,
};

use lexiswitch_core::state::AgentState;

use crate::WinError;

/// The icon for a state.
///
/// Three icons, not five. A condition the user must act on is worth distinguishing from one
/// they chose, and from everything working; which particular condition it is belongs in the
/// tooltip, where there is room to say it.
fn load_icon(state: &AgentState) -> Result<HICON, WinError> {
    let which = if state.needs_attention() {
        IDI_WARNING
    } else if state.converts() {
        IDI_APPLICATION
    } else {
        IDI_HAND
    };
    // SAFETY: both are built-in resource identifiers, so no module handle is needed.
    unsafe { LoadIconW(None, which) }.map_err(|e| WinError::new("LoadIconW", e))
}

/// Message the tray icon posts back to the window for clicks.
///
/// The icon uses the **legacy** protocol: `wParam` is the icon id and `lParam` is the mouse
/// message. `NIM_SETVERSION` with `NOTIFYICON_VERSION_4` was tried and reverted, because it
/// costs the tooltip. Under version 4 the shell stops drawing `szTip` and instead sends
/// `NIN_POPUPOPEN` so the application can render its own — measured directly with
/// `examples/probe_tray.rs`, which registered three icons side by side and watched what the
/// shell sent each: the version 4 icon received seven `NIN_POPUPOPEN` events and showed no
/// tooltip, while both legacy icons showed theirs.
///
/// Version 4 buys nothing here in exchange. The measurement also settled the question it was
/// introduced to answer: the shell delivers `WM_RBUTTONUP` under *either* protocol, including
/// to a message-only window. Right-clicks were never being lost.
pub const WM_TRAY: u32 = WM_APP + 1;

/// The notification event out of a [`WM_TRAY`] message's `lParam`.
///
/// Under the legacy protocol the whole `lParam` is the event, so the mask is a no-op — it is
/// kept because it is also correct under version 4, where the high word carries the icon id
/// and a bare comparison silently never matches.
pub fn tray_event(lparam: isize) -> u32 {
    (lparam as u32) & 0xFFFF
}

const ICON_ID: u32 = 1;

/// A tray icon, removed when dropped.
pub struct Tray {
    hwnd: HWND,
}

/// Menu command identifiers, posted back as `WM_COMMAND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    EnableDisable = 100,
    ReloadConfig = 101,
    OpenConfig = 102,
    Quit = 103,
    ConvertSelection = 104,
}

impl MenuCommand {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            100 => Some(Self::EnableDisable),
            101 => Some(Self::ReloadConfig),
            102 => Some(Self::OpenConfig),
            104 => Some(Self::ConvertSelection),
            103 => Some(Self::Quit),
            _ => None,
        }
    }
}

impl Tray {
    /// Create the icon showing `state`.
    ///
    /// The state is passed in rather than assumed, because the tray now appears in conditions
    /// that used to end the process before it existed — a held shortcut, or too few layouts.
    /// Coming up in the enabled icon and correcting itself afterwards would flash the wrong
    /// answer at exactly the moment the user is being told something is wrong.
    pub fn new(hwnd: HWND, state: &AgentState) -> Result<Self, WinError> {
        let mut data = Self::base(hwnd);
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.hIcon = load_icon(state)?;
        write_wide(&mut data.szTip, &state.summary());

        // SAFETY: `data` is fully initialised and its cbSize field matches the struct.
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &data)
                .ok()
                .map_err(|e| WinError::new("Shell_NotifyIconW(NIM_ADD)", e))?;
        }
        Ok(Self { hwnd })
    }

    /// Switch between the enabled and disabled appearances.
    ///
    /// Disabled uses the system's "no entry" icon rather than a dimmed variant of the enabled
    /// one: a tray icon is 16 pixels, and whether the hotkey will do anything should be
    /// readable at a glance rather than by comparison.
    ///
    /// This used to mean paused *recording*. Nothing is recorded now, so it means only that
    /// the hotkey does not convert — a distinction the tooltip has to make, because a user
    /// who remembers the old behaviour will otherwise read "disabled" as "no longer
    /// watching", which was never something they had to turn off.
    pub fn set_state(&self, state: &AgentState) {
        let Ok(icon) = load_icon(state) else { return };
        let mut data = Self::base(self.hwnd);
        // NIF_MESSAGE on every modify, or this call silently unhooks the icon.
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.hIcon = icon;
        // The state writes its own tooltip, including the active shortcut where there is one:
        // a tray icon that does not say how to invoke the program teaches nothing.
        write_wide(&mut data.szTip, &state.summary());
        // SAFETY: modifies the icon this type added.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    /// Show the context menu at the cursor and return the chosen command.
    ///
    /// `SetForegroundWindow` before `TrackPopupMenu` is not optional: without it the menu
    /// does not dismiss when the user clicks elsewhere, and stays on screen forever. This is
    /// a documented quirk of tray menus.
    pub fn show_menu(&self, state: &AgentState) -> Option<MenuCommand> {
        // SAFETY: the menu is created, used and destroyed within this call. Every pointer
        // passed is to a local that outlives the call.
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            // Conversion from the menu, first because it is the only way to convert while the
            // shortcut is held by another application. Not a convenience: without it a conflict
            // leaves a program that runs and can do nothing at all.
            let _ = AppendMenuW(
                menu,
                if state.converts() { MF_STRING } else { MF_GRAYED },
                MenuCommand::ConvertSelection as usize,
                w!("Convert selection"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

            let toggle = if matches!(state, AgentState::Disabled) {
                w!("Enable LexiSwitch")
            } else {
                w!("Disable LexiSwitch")
            };
            let _ = AppendMenuW(menu, MF_STRING, MenuCommand::EnableDisable as usize, toggle);
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MenuCommand::ReloadConfig as usize,
                w!("Reload configuration"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MenuCommand::OpenConfig as usize,
                w!("Open configuration file"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, MenuCommand::Quit as usize, w!("Quit"));

            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);
            let _ = SetForegroundWindow(self.hwnd);

            let chosen = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                Some(0),
                self.hwnd,
                None,
            );
            // The other half of the foreground dance, and as load-bearing as the first: without
            // a message posted to the owner afterwards, the *next* menu fails to dismiss when
            // the user clicks elsewhere and stays on screen.
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            MenuCommand::from_raw(chosen.0 as u32)
        }
    }

    /// Show a balloon. Failures are swallowed: a notification that cannot be shown must not
    /// take down whatever it was reporting on.
    pub fn notify(&self, title: &str, body: &str) {
        let mut data = Self::base(self.hwnd);
        // NIF_MESSAGE on every modify, or this call silently unhooks the icon.
        data.uFlags = NIF_INFO | NIF_MESSAGE;
        data.dwInfoFlags = NOTIFY_ICON_INFOTIP_FLAGS(0);
        write_wide(&mut data.szInfoTitle, title);
        write_wide(&mut data.szInfo, body);

        // SAFETY: same shape as NIM_ADD, on an icon this type added and has not removed.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    /// The fields every call shares.
    ///
    /// `uCallbackMessage` is set here rather than only on `NIM_ADD`, and every caller ORs
    /// `NIF_MESSAGE` into its flags, because the alternative was measured to break the icon.
    /// `..Default::default()` zeroes the callback, so a `NIM_MODIFY` that did not re-assert it
    /// left the shell with no message to send — and the very first thing this program does
    /// after adding its icon is show a balloon, which is a `NIM_MODIFY`. The result was an icon
    /// with a working tooltip that silently swallowed every click, because the tooltip is drawn
    /// from data the shell already holds and needs no callback at all.
    fn base(hwnd: HWND) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ICON_ID,
            uCallbackMessage: WM_TRAY,
            // Left at zero: the legacy protocol. See `new` for why version 4 was tried and
            // taken back out.
            Anonymous: NOTIFYICONDATAW_0 { uVersion: 0 },
            ..Default::default()
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let data = Self::base(self.hwnd);
        // SAFETY: removes the icon this type added; harmless if it is already gone.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }
}

/// Copy `text` into a fixed wide buffer, NUL-terminated and truncated to fit.
///
/// Truncation is at a character boundary rather than a UTF-16 unit, so a clipped message
/// cannot end in half a surrogate pair.
fn write_wide(dst: &mut [u16], text: &str) {
    let limit = dst.len().saturating_sub(1);
    let mut n = 0;
    for c in text.chars() {
        let len = c.len_utf16();
        if n + len > limit {
            break;
        }
        let mut units = [0u16; 2];
        for (i, unit) in c.encode_utf16(&mut units).iter().enumerate() {
            dst[n + i] = *unit;
        }
        n += len;
    }
    dst[n] = 0;
    let _ = w!("");
}
