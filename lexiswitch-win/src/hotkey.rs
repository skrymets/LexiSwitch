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
///
/// The numbering has gaps. Ids 1, 3 and 4 belonged to the last-word, clipboard and pause
/// hotkeys, which retired with keyboard observation
/// ([ADR-0011](../../docs/adr/0011-no-keyboard-observation.md)); the survivor keeps its value
/// rather than being renumbered, because these are only ever compared against themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HotkeyId {
    ConvertSelection = 2,
    ChangeCase = 5,
    Transliterate = 6,
}

impl HotkeyId {
    pub fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            2 => Some(Self::ConvertSelection),
            5 => Some(Self::ChangeCase),
            6 => Some(Self::Transliterate),
            _ => None,
        }
    }

    /// The configuration key this hotkey comes from, for error messages.
    pub fn config_key(&self) -> &'static str {
        match self {
            Self::ConvertSelection => "convert_selection",
            Self::ChangeCase => "change_case",
            Self::Transliterate => "transliterate",
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
/// Registration is **per binding**. It was all-or-nothing while there was only one binding, and
/// that was the right shape for one: a set where half the actions are silently dead is worse
/// than none. With two it becomes actively harmful — a combination held by another application
/// would release the one that registered perfectly well, so a conflict on the secondary
/// shortcut would disable the primary one. Each binding now succeeds or fails on its own and
/// the caller is told which did which
/// ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)).
pub struct Hotkeys {
    hwnd: HWND,
    registered: Vec<HotkeyId>,
}

/// What a registration attempt achieved, and what it did not.
///
/// Not a `Result`: partial success is the normal outcome now, and expressing it as an error
/// would push every caller into treating one unavailable combination as a failure of the whole
/// set — which is the behaviour being removed.
pub struct Registration {
    /// The bindings that took. Releases them when dropped.
    pub hotkeys: Hotkeys,
    /// The bindings that did not, each naming itself.
    pub failed: Vec<HotkeyError>,
}

impl Hotkeys {
    /// Try every binding, keeping whatever works.
    pub fn register(hwnd: HWND, bindings: &[(HotkeyId, Binding)]) -> Registration {
        let mut hotkeys = Self {
            hwnd,
            registered: Vec::new(),
        };
        let mut failed = Vec::new();
        for (id, binding) in bindings {
            // SAFETY: hwnd is a live window owned by this thread, and the id is unique
            // within it. On failure nothing was registered for this id.
            let result =
                unsafe { RegisterHotKey(Some(hwnd), *id as i32, binding.modifiers, binding.vkey) };
            match result {
                Ok(()) => hotkeys.registered.push(*id),
                Err(e) => failed.push(HotkeyError {
                    id: *id,
                    source: WinError::new("RegisterHotKey", e),
                }),
            }
        }
        Registration { hotkeys, failed }
    }

    /// Whether this set holds a live registration for `id`.
    pub fn holds(&self, id: HotkeyId) -> bool {
        self.registered.contains(&id)
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

/// Render a virtual-key code and modifier state as text [`parse_binding`] accepts.
///
/// The inverse of the table above, and deliberately written next to it: the two must agree, and
/// a name produced here that cannot be parsed back would be written into the user's
/// configuration file and rejected on the next load.
///
/// Returns `None` for a key with no name, so a caller recording a combination can decline it
/// rather than persist something unusable.
pub fn format_binding(modifiers: HOT_KEY_MODIFIERS, vkey: u32) -> Option<String> {
    let key = key_name(vkey)?;
    let mut parts = Vec::new();
    // A fixed order, so the same combination always renders the same way. The user's file
    // should not gain a diff because they pressed Alt before Ctrl.
    if modifiers & MOD_CONTROL != HOT_KEY_MODIFIERS(0) {
        parts.push("Ctrl");
    }
    if modifiers & MOD_ALT != HOT_KEY_MODIFIERS(0) {
        parts.push("Alt");
    }
    if modifiers & MOD_SHIFT != HOT_KEY_MODIFIERS(0) {
        parts.push("Shift");
    }
    if modifiers & MOD_WIN != HOT_KEY_MODIFIERS(0) {
        parts.push("Win");
    }
    parts.push(&key);
    Some(parts.join("+"))
}

/// The canonical name for a virtual-key code, or `None` if it has none.
fn key_name(vkey: u32) -> Option<String> {
    // Only these two ranges are the ASCII characters they look like. Testing
    // `char::from_u32(vkey).is_ascii_alphanumeric()` instead is wrong and quietly so: VK_F1 is
    // 0x70, which is ASCII 'p', so F1 rendered as "P" and parsed back as the letter.
    if (0x30..=0x39).contains(&vkey) || (0x41..=0x5A).contains(&vkey) {
        if let Some(c) = char::from_u32(vkey) {
            return Some(c.to_ascii_uppercase().to_string());
        }
    }
    Some(
        match vkey {
            0x13 => "Pause",
            0x20 => "Space",
            0x09 => "Tab",
            0x0D => "Enter",
            0x1B => "Escape",
            0x2D => "Insert",
            0x2E => "Delete",
            0x24 => "Home",
            0x23 => "End",
            0x21 => "PageUp",
            0x22 => "PageDown",
            0x25 => "Left",
            0x26 => "Up",
            0x27 => "Right",
            0x28 => "Down",
            0x70..=0x7B => return Some(format!("F{}", vkey - 0x70 + 1)),
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name this produces must parse back to what produced it. A binding written to the
    /// user's file that the loader then rejects is the worst of both worlds.
    #[test]
    fn formatting_round_trips_through_parsing() {
        let cases: &[(HOT_KEY_MODIFIERS, u32)] = &[
            (MOD_CONTROL | MOD_ALT, b'K' as u32),
            (MOD_SHIFT, 0x13),
            (HOT_KEY_MODIFIERS(0), 0x70),
            (MOD_WIN | MOD_CONTROL, 0x7B),
            (MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_WIN, b'9' as u32),
            (MOD_ALT, 0x22),
        ];
        for (modifiers, vkey) in cases {
            let text = format_binding(*modifiers, *vkey).expect("a nameable key");
            let parsed = parse_binding(&text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
            assert_eq!(parsed.modifiers, *modifiers, "{text:?}");
            assert_eq!(parsed.vkey, *vkey, "{text:?}");
        }
    }

    /// Modifier order is fixed, not the order they were pressed in.
    #[test]
    fn modifiers_render_in_a_fixed_order() {
        let a = format_binding(MOD_ALT | MOD_CONTROL, b'K' as u32).unwrap();
        let b = format_binding(MOD_CONTROL | MOD_ALT, b'K' as u32).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "Ctrl+Alt+K");
    }

    /// A key with no name is declined rather than rendered as something unparseable.
    #[test]
    fn an_unnameable_key_is_declined() {
        assert_eq!(format_binding(MOD_CONTROL, 0xFF), None);
    }

    /// One combination another application holds must not take the others down with it.
    ///
    /// The failure this guards against is invisible from inside: a conflict on the case
    /// shortcut would have released the conversion shortcut, so the product's primary function
    /// would stop working because its secondary one could not be bound.
    ///
    /// A null window handle registers against the calling thread, which is what makes this
    /// testable without creating a window. The combinations are deliberately absurd — every
    /// modifier plus a key most keyboards do not have — so the test does not depend on what
    /// else is running, and both registrations are released when the values drop.
    #[test]
    fn an_unavailable_binding_does_not_release_the_available_ones() {
        let hwnd = HWND(std::ptr::null_mut());
        let all = MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_WIN;
        let contested = Binding {
            modifiers: all,
            vkey: 0x87,
        };
        let free = Binding {
            modifiers: all,
            vkey: 0x86,
        };

        let holder = Hotkeys::register(hwnd, &[(HotkeyId::ConvertSelection, contested)]);
        assert!(
            holder.failed.is_empty(),
            "this test needs the contested combination to start out free"
        );

        // The same id against the same window, so the refusal is certain rather than
        // dependent on what else happens to be running.
        let second = Hotkeys::register(
            hwnd,
            &[
                (HotkeyId::ChangeCase, free),
                (HotkeyId::ConvertSelection, contested),
            ],
        );

        assert!(
            second.hotkeys.holds(HotkeyId::ChangeCase),
            "the available binding was released along with the unavailable one"
        );
        assert_eq!(second.failed.len(), 1, "exactly one binding was unavailable");
        assert_eq!(second.failed[0].id, HotkeyId::ConvertSelection);
    }
}
