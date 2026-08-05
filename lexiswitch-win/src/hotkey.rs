//! Global hotkeys, and classifying repeated presses into multi-tap runs.
//!
//! `RegisterHotKey` consumes the hotkey itself, so no keyboard hook is needed to trigger
//! anything ([ADR-0003](../../docs/adr/0003-raw-input-over-low-level-hook.md)).
//!
//! A conflict — another application already holding the combination — fails visibly. The
//! spike confirmed the failure is `ERROR_HOTKEY_ALREADY_REGISTERED` (`0x80070581`) with a
//! message already fit to show a user, so startup can say which binding is unavailable
//! rather than dying with a code.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
};

use crate::WinError;

/// Which action a registered hotkey triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HotkeyId {
    ConvertLastWord = 1,
    ConvertSelection = 2,
    ConvertClipboard = 3,
    PauseToggle = 4,
}

impl HotkeyId {
    pub fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            1 => Some(Self::ConvertLastWord),
            2 => Some(Self::ConvertSelection),
            3 => Some(Self::ConvertClipboard),
            4 => Some(Self::PauseToggle),
            _ => None,
        }
    }

    /// The configuration key this hotkey comes from, for error messages.
    pub fn config_key(&self) -> &'static str {
        match self {
            Self::ConvertLastWord => "convert_last_word",
            Self::ConvertSelection => "convert_selection",
            Self::ConvertClipboard => "convert_clipboard",
            Self::PauseToggle => "pause_toggle",
        }
    }
}

/// A parsed binding: modifiers plus one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub vkey: u32,
}

/// Parse a binding such as `"Ctrl+Shift+Pause"`.
///
/// Case-insensitive, tolerant of spaces around the separators. The key must come last and
/// there must be exactly one of it.
pub fn parse_binding(text: &str) -> Result<Binding, String> {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut key: Option<u32> = None;

    for part in text.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("{text:?} has an empty component"));
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" | "super" => modifiers |= MOD_WIN,
            other => {
                if key.is_some() {
                    return Err(format!(
                        "{text:?} names more than one key; a binding is zero or more \
                         modifiers plus exactly one key"
                    ));
                }
                key = Some(virtual_key(other).ok_or_else(|| format!("unknown key {part:?}"))?);
            }
        }
    }

    match key {
        Some(vkey) => Ok(Binding { modifiers, vkey }),
        None => Err(format!("{text:?} names modifiers but no key")),
    }
}

/// Map a key name to its virtual-key code.
fn virtual_key(name: &str) -> Option<u32> {
    // Single characters map directly for A-Z and 0-9.
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase() as u32);
        }
    }
    Some(match name {
        "pause" | "break" => 0x13,
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "insert" => 0x2D,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        _ => return None,
    })
}

/// Registered hotkeys, released when dropped.
///
/// Registration is all-or-nothing: if any binding is unavailable, everything already
/// registered is released and the error names which one failed. A half-registered set would
/// leave some actions silently dead.
pub struct Hotkeys {
    hwnd: HWND,
    registered: Vec<HotkeyId>,
}

impl Hotkeys {
    pub fn register(hwnd: HWND, bindings: &[(HotkeyId, Binding)]) -> Result<Self, HotkeyError> {
        let mut this = Self {
            hwnd,
            registered: Vec::new(),
        };
        for (id, binding) in bindings {
            // SAFETY: hwnd is a live window owned by this thread, and the id is unique
            // within it. On failure nothing was registered for this id.
            let result = unsafe {
                RegisterHotKey(Some(hwnd), *id as i32, binding.modifiers, binding.vkey)
            };
            match result {
                Ok(()) => this.registered.push(*id),
                Err(e) => {
                    // Dropping `this` releases whatever did register.
                    return Err(HotkeyError {
                        id: *id,
                        source: WinError::new("RegisterHotKey", e),
                    });
                }
            }
        }
        Ok(this)
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        for id in &self.registered {
            // SAFETY: each id was successfully registered against this window and is
            // unregistered exactly once.
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), *id as i32);
            }
        }
    }
}

/// A binding that could not be registered, and which one it was.
#[derive(Debug, Clone)]
pub struct HotkeyError {
    pub id: HotkeyId,
    pub source: WinError,
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hotkey {} could not be registered: {}",
            self.id.config_key(),
            self.source.message()
        )
    }
}

impl std::error::Error for HotkeyError {}
