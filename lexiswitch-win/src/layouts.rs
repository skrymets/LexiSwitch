//! Building layout tables from the layouts Windows actually has installed.
//!
//! Per [ADR-0004](../../docs/adr/0004-runtime-derived-layout-tables.md) nothing is compiled
//! in: the tables come from `GetKeyboardLayoutList` plus `ToUnicodeEx` at startup, and are
//! immutable afterwards. A layout installed later needs a restart, which is an acceptable
//! cost for a table that is otherwise read-only and shareable across threads without
//! synchronisation.
//!
//! Two things here were established by the Phase 0 spike and are not obvious:
//!
//! - `ToUnicodeEx` is stateful. Probing a dead key leaves the accent pending inside the
//!   layout, and the *next* probe silently returns a combined character with no error. Every
//!   probe is preceded by a flush for that reason.
//! - The HKL high word is not the layout identifier. Ukrainian (Enhanced) reports
//!   `0xf0a80422`, so only the low word is the language id.

use lexiswitch_core::layout::{KeyPos, KeyValue, LayoutTable};
use windows::Win32::Globalization::LCIDToLocaleName;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, MapVirtualKeyExW, ToUnicodeEx, HKL, MAPVK_VK_TO_VSC,
    MAPVK_VSC_TO_VK_EX,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
};

use crate::WinError;

/// The alphanumeric and punctuation block, in set-1 scancodes. Function keys, the keypad and
/// navigation keys are deliberately outside the conversion domain.
const SCANCODES: &[u16] = &[
    0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, // number row
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, // top row
    0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, // home row
    0x29, 0x2B, // key left of 1, and backslash
    0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, // bottom row
    0x56, // ISO extra key
];

const VK_SHIFT: usize = 0x10;
const VK_SPACE: u32 = 0x20;

/// A layout Windows has installed, with its derived table.
#[derive(Debug, Clone)]
pub struct InstalledLayout {
    /// Raw HKL value, as reported, at its full width.
    ///
    /// Deliberately not a `u32`. A layout that is not the default for its language gets a
    /// handle whose high word is a device identifier rather than a language id, and on 64-bit
    /// those come back sign-extended — Ukrainian reports `0xFFFFFFFF_F0A80422` here. Narrowing
    /// to 32 bits and widening again produced `0x00000000_F0A80422`, a handle naming no
    /// installed layout, so every request to activate it was silently ignored while the tidy
    /// handles of en-US and de-DE kept working.
    pub hkl: usize,
    /// Language identifier — the *low word* of the HKL.
    pub langid: u16,
    /// BCP-47 name, such as `de-DE`.
    pub name: String,
    pub table: LayoutTable,
}

/// Build a table for every installed layout.
///
/// Fails when fewer than two layouts are installed: with one layout there is nothing to
/// convert between, and starting anyway would leave a tray icon that can never do anything.
pub fn build_all() -> Result<Vec<InstalledLayout>, LayoutError> {
    let hkls = installed_hkls()?;
    if hkls.len() < 2 {
        return Err(LayoutError::TooFewLayouts(hkls.len()));
    }

    Ok(hkls
        .into_iter()
        .map(|hkl| {
            let raw = hkl_raw(hkl);
            let langid = (raw & 0xFFFF) as u16;
            InstalledLayout {
                hkl: raw,
                langid,
                name: locale_name(langid),
                table: probe(hkl, &locale_name(langid)),
            }
        })
        .collect())
}

/// The language identifier of the layout receiving the user's keystrokes.
///
/// Conversion has to resolve the buffer against the layout that was active *while typing*,
/// not this process's own — they are usually different, since the whole point is that the
/// user was typing somewhere else.
///
/// The window that has keyboard focus is not always the foreground window. Packaged
/// applications — Windows 11's Notepad among them — put an `ApplicationFrameWindow` owned by
/// `ApplicationFrameHost.exe` in the foreground while a child window in the real process
/// receives input. Asking the foreground window's thread therefore reports the *host's*
/// layout, which is whatever it happens to be and not what the user is typing with.
///
/// `GetGUIThreadInfo` on the foreground thread reports the focused window, which is the one
/// that matters. Where that is unavailable the foreground window is used, which is correct
/// for ordinary Win32 applications.
pub fn foreground_langid() -> u16 {
    // SAFETY: GUITHREADINFO is initialised with its own size as the API requires. A zero
    // thread id asks about the foreground thread. Every handle involved may be null, which
    // GetWindowThreadProcessId accepts, and GetKeyboardLayout(0) then answers with this
    // thread's layout — a defined fallback rather than an error.
    unsafe {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focused = if GetGUIThreadInfo(0, &mut info).is_ok() && !info.hwndFocus.is_invalid() {
            info.hwndFocus
        } else {
            GetForegroundWindow()
        };
        let thread = GetWindowThreadProcessId(focused, None);
        let hkl = GetKeyboardLayout(thread);
        (hkl_raw(hkl) & 0xFFFF) as u16
    }
}

/// Diagnostic detail about where keyboard input is going, for investigating layout misreads.
///
/// Not part of the product's behaviour — it exists so a wrong answer from
/// [`foreground_langid`] can be attributed to the right cause rather than guessed at.
pub fn describe_focus() -> String {
    // SAFETY: as `foreground_langid`. GetClassNameW writes at most the buffer length it is
    // given and returns how much it wrote.
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let fg_lang = (hkl_raw(GetKeyboardLayout(fg_thread)) & 0xFFFF) as u16;

        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let ok = GetGUIThreadInfo(0, &mut info).is_ok();
        let focus = info.hwndFocus;
        let focus_thread = GetWindowThreadProcessId(focus, None);
        let focus_lang = (hkl_raw(GetKeyboardLayout(focus_thread)) & 0xFFFF) as u16;

        let mut class = [0u16; 128];
        let n = GetClassNameW(fg, &mut class);
        let fg_class = String::from_utf16_lossy(&class[..n as usize]);

        let mut fclass = [0u16; 128];
        let fnlen = GetClassNameW(focus, &mut fclass);
        let focus_class = String::from_utf16_lossy(&fclass[..fnlen as usize]);

        format!(
            "foreground[class={fg_class:?} thread={fg_thread} lang={fg_lang:#06x}] \
             guiInfo={ok} focus[class={focus_class:?} thread={focus_thread} lang={focus_lang:#06x}]"
        )
    }
}

fn installed_hkls() -> Result<Vec<HKL>, LayoutError> {
    // SAFETY: the first call passes no buffer and only asks for the count; the second is
    // given a buffer of exactly that length, and the return value is how many were written.
    unsafe {
        let count = GetKeyboardLayoutList(None);
        if count <= 0 {
            return Err(LayoutError::Win(WinError::last("GetKeyboardLayoutList")));
        }
        let mut list = vec![HKL::default(); count as usize];
        let written = GetKeyboardLayoutList(Some(&mut list));
        if written <= 0 {
            return Err(LayoutError::Win(WinError::last("GetKeyboardLayoutList")));
        }
        list.truncate(written as usize);
        Ok(list)
    }
}

fn hkl_raw(hkl: HKL) -> usize {
    hkl.0 as usize
}

fn locale_name(langid: u16) -> String {
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
                              // SAFETY: buf is the documented maximum size, and its length is passed with it.
    let n = unsafe { LCIDToLocaleName(langid as u32, Some(&mut buf), 0) };
    if n <= 0 {
        return format!("{langid:#06x}");
    }
    // The count includes the terminating NUL.
    String::from_utf16_lossy(&buf[..(n as usize - 1)])
}

/// Clear any pending dead-key state so the next probe starts clean.
///
/// Pressing a known non-dead key consumes a pending accent. The spike proved this both
/// necessary and sufficient: probing German `^` then `e` without flushing returns `ê`.
fn flush(hkl: HKL) {
    let state = [0u8; 256];
    let mut buf = [0u16; 16];
    // SAFETY: `state` is the required 256 bytes and `buf`'s length travels with it. Looping
    // is bounded, so a layout that never returns >= 0 cannot hang startup.
    unsafe {
        let sc = MapVirtualKeyExW(VK_SPACE, MAPVK_VK_TO_VSC, Some(hkl));
        for _ in 0..8 {
            if ToUnicodeEx(VK_SPACE, sc, &state, &mut buf, 0, Some(hkl)) >= 0 {
                break;
            }
        }
    }
}

fn probe(hkl: HKL, name: &str) -> LayoutTable {
    let mut entries: Vec<(KeyPos, KeyValue)> = Vec::with_capacity(SCANCODES.len() * 2);

    for &scancode in SCANCODES {
        for shift in [false, true] {
            flush(hkl);

            let mut state = [0u8; 256];
            if shift {
                state[VK_SHIFT] = 0x80;
            }
            let mut buf = [0u16; 16];

            // SAFETY: `state` is 256 bytes as required and `buf`'s length is passed with
            // it. The Ex variants resolve against `hkl` rather than the active layout,
            // which is the entire reason they are used here.
            let (vk, written) = unsafe {
                let vk = MapVirtualKeyExW(scancode as u32, MAPVK_VSC_TO_VK_EX, Some(hkl));
                if vk == 0 {
                    continue;
                }
                (
                    vk,
                    ToUnicodeEx(vk, scancode as u32, &state, &mut buf, 0, Some(hkl)),
                )
            };
            let _ = vk;

            let pos = KeyPos::new(scancode, shift);
            match written {
                // Dead key. Recorded as dead rather than resolved — it has no single
                // character, and inventing one would poison every later conversion.
                -1 => {
                    entries.push((pos, KeyValue::Dead));
                    flush(hkl);
                }
                0 => {}
                n => {
                    let s = String::from_utf16_lossy(&buf[..n as usize]);
                    let mut chars = s.chars();
                    if let (Some(c), None) = (chars.next(), chars.next()) {
                        entries.push((pos, KeyValue::Char(c)));
                    }
                    // More than one character from a single position cannot participate in
                    // a character-to-position inverse, so it is left out rather than
                    // truncated to its first character.
                }
            }
        }
    }

    LayoutTable::new(name, entries)
}

/// Why layout tables could not be built.
#[derive(Debug)]
pub enum LayoutError {
    /// Conversion needs something to convert between.
    TooFewLayouts(usize),
    Win(WinError),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewLayouts(n) => write!(
                f,
                "LexiSwitch needs at least two keyboard layouts installed; found {n}. \
                 Add one under Settings > Time & language > Language & region."
            ),
            Self::Win(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LayoutError {}
