//! The tray icon and its notifications.
//!
//! There is no console and no log by default, so a failure that does not reach the tray is
//! invisible. The `agent-control` spec requires notifications to state what did *not*
//! happen — "clipboard busy, selection not converted", never "error 5".
//!
//! Menu and paused/active appearances arrive with tasks 9.3 and 9.4; this is the icon and
//! the balloon.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFY_ICON_INFOTIP_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow,
    TrackPopupMenu, HICON, IDI_APPLICATION, IDI_HAND, MF_SEPARATOR, MF_STRING, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
};

use crate::WinError;

/// The active or paused icon.
fn load_icon(paused: bool) -> Result<HICON, WinError> {
    let which = if paused { IDI_HAND } else { IDI_APPLICATION };
    // SAFETY: both are built-in resource identifiers, so no module handle is needed.
    unsafe { LoadIconW(None, which) }.map_err(|e| WinError::new("LoadIconW", e))
}

/// Message the tray icon posts back to the window for clicks.
pub const WM_TRAY: u32 = WM_APP + 1;

const ICON_ID: u32 = 1;

/// A tray icon, removed when dropped.
pub struct Tray {
    hwnd: HWND,
}

/// Menu command identifiers, posted back as `WM_COMMAND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    PauseResume = 100,
    ReloadConfig = 101,
    OpenConfig = 102,
    Quit = 103,
}

impl MenuCommand {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            100 => Some(Self::PauseResume),
            101 => Some(Self::ReloadConfig),
            102 => Some(Self::OpenConfig),
            103 => Some(Self::Quit),
            _ => None,
        }
    }
}

impl Tray {
    pub fn new(hwnd: HWND, tooltip: &str) -> Result<Self, WinError> {
        let mut data = Self::base(hwnd);
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = load_icon(false)?;
        write_wide(&mut data.szTip, tooltip);

        // SAFETY: `data` is fully initialised and its cbSize field matches the struct.
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &data)
                .ok()
                .map_err(|e| WinError::new("Shell_NotifyIconW(NIM_ADD)", e))?;
        }
        Ok(Self { hwnd })
    }

    /// Switch between the active and paused appearances.
    ///
    /// Paused uses the system's "no entry" icon rather than a dimmed variant of the active
    /// one: a tray icon is 16 pixels, and a state that matters this much — keystrokes are or
    /// are not being recorded — should be distinguishable at a glance, not by comparison.
    pub fn set_paused(&self, paused: bool) {
        let Ok(icon) = load_icon(paused) else { return };
        let mut data = Self::base(self.hwnd);
        data.uFlags = NIF_ICON | NIF_TIP;
        data.hIcon = icon;
        write_wide(
            &mut data.szTip,
            if paused {
                "LexiSwitch — paused, not recording"
            } else {
                "LexiSwitch"
            },
        );
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
    pub fn show_menu(&self, paused: bool) -> Option<MenuCommand> {
        // SAFETY: the menu is created, used and destroyed within this call. Every pointer
        // passed is to a local that outlives the call.
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let pause_label = if paused { w!("Resume") } else { w!("Pause") };
            let _ = AppendMenuW(menu, MF_STRING, MenuCommand::PauseResume as usize, pause_label);
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
            let _ = DestroyMenu(menu);
            MenuCommand::from_raw(chosen.0 as u32)
        }
    }

    /// Show a balloon. Failures are swallowed: a notification that cannot be shown must not
    /// take down whatever it was reporting on.
    pub fn notify(&self, title: &str, body: &str) {
        let mut data = Self::base(self.hwnd);
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NOTIFY_ICON_INFOTIP_FLAGS(0);
        write_wide(&mut data.szInfoTitle, title);
        write_wide(&mut data.szInfo, body);

        // SAFETY: same shape as NIM_ADD, on an icon this type added and has not removed.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    fn base(hwnd: HWND) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ICON_ID,
            Anonymous: NOTIFYICONDATAW_0 { uVersion: 4 },
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
