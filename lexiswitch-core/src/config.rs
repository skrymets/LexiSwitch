//! Configuration: types, defaults, and validation.
//!
//! A configuration is parsed and validated once, then treated as immutable. A reload builds
//! a whole new one and swaps it wholesale, so nothing ever reads half-old, half-new state —
//! and a file that fails validation simply does not get swapped in, which is how
//! last-known-good works without a rollback path.
//!
//! Unknown keys are rejected rather than ignored. A typo that is silently dropped looks
//! exactly like a setting that does not work, and the user has no way to tell the
//! difference.

use serde::Deserialize;

/// The whole configurable surface, in one readable place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub hotkeys: Hotkeys,
    pub layouts: Layouts,
    pub buffer: BufferConfig,
    pub selection: Selection,
    pub exclusions: Exclusions,
    pub behavior: Behavior,
    pub logging: Logging,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Hotkeys {
    pub convert_last_word: String,
    pub convert_selection: String,
    pub convert_clipboard: String,
    pub pause_toggle: String,
    pub multi_tap_window_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Layouts {
    /// Cycle order by language tag. Empty means installed order, layout active at typing
    /// time first.
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BufferConfig {
    pub idle_timeout_secs: u32,
    /// Refusing to record in password fields. Validation rejects `false` — see
    /// [`Config::validate`].
    pub respect_password_fields: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Selection {
    pub allow_clipboard_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Exclusions {
    pub processes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Behavior {
    /// Activate the layout the text was converted into, after a correction.
    ///
    /// Off unless enabled, which is also the derived default — the two must not drift
    /// apart, so this relies on the derive rather than restating it. It makes the *next*
    /// word land in a different layout from the one just corrected, which is disorienting
    /// when fixing several words in a row.
    pub switch_layout_after_correction: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Logging {
    /// Off unless explicitly enabled, which is also the derived default — the two must not
    /// drift apart, so this relies on the derive rather than restating it.
    pub enabled: bool,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            convert_last_word: "Pause".into(),
            convert_selection: "Shift+Pause".into(),
            convert_clipboard: "Ctrl+Pause".into(),
            pause_toggle: "Ctrl+Shift+Pause".into(),
            multi_tap_window_ms: 500,
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 30,
            respect_password_fields: true,
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            allow_clipboard_fallback: true,
        }
    }
}

impl Default for Exclusions {
    fn default() -> Self {
        Self {
            processes: vec![
                // Password managers: a detection failure matters most here.
                "KeePassXC.exe".into(),
                "1Password.exe".into(),
                "Bitwarden.exe".into(),
                // Terminals: a password prompt there is undetectable in principle — there
                // is no control with an IsPassword property, only a shell that stopped
                // echoing. Measured; see docs/manual-tests.md.
                "WindowsTerminal.exe".into(),
            ],
        }
    }
}

/// The multi-tap window cannot usefully go below the message-timestamp granularity, which
/// the Phase 0 spike measured at 15.625 ms.
const MIN_MULTI_TAP_MS: u32 = 50;
const MAX_MULTI_TAP_MS: u32 = 5_000;
const MAX_IDLE_SECS: u32 = 3_600;

impl Config {
    /// Parse and validate. Either the whole configuration is good, or nothing is returned.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(text).map_err(|e| ConfigError {
            message: e.message().to_string(),
            line: e.span().map(|s| line_of_offset(text, s.start)),
        })?;
        config.validate(text)?;
        Ok(config)
    }

    /// Rules that TOML itself cannot express.
    ///
    /// Each failure carries the line the offending setting sits on, because "your config is
    /// wrong" without a location is a worse experience than no message at all.
    fn validate(&self, text: &str) -> Result<(), ConfigError> {
        // The one setting that exists in the schema so its absence cannot be mistaken for
        // an oversight, and is refused rather than honoured (ADR-0005).
        if !self.buffer.respect_password_fields {
            return Err(ConfigError::at(
                text,
                "buffer",
                "respect_password_fields",
                "respect_password_fields cannot be disabled: LexiSwitch will not record \
                 keystrokes in password fields. Use [exclusions] to exempt an application \
                 entirely.",
            ));
        }

        if !(MIN_MULTI_TAP_MS..=MAX_MULTI_TAP_MS).contains(&self.hotkeys.multi_tap_window_ms) {
            return Err(ConfigError::at(
                text,
                "hotkeys",
                "multi_tap_window_ms",
                &format!(
                    "multi_tap_window_ms must be between {MIN_MULTI_TAP_MS} and \
                     {MAX_MULTI_TAP_MS}; message timestamps resolve to about 16 ms, so \
                     smaller values cannot be told apart"
                ),
            ));
        }

        if self.buffer.idle_timeout_secs == 0 || self.buffer.idle_timeout_secs > MAX_IDLE_SECS {
            return Err(ConfigError::at(
                text,
                "buffer",
                "idle_timeout_secs",
                &format!("idle_timeout_secs must be between 1 and {MAX_IDLE_SECS}"),
            ));
        }

        for (key, value) in [
            ("convert_last_word", &self.hotkeys.convert_last_word),
            ("convert_selection", &self.hotkeys.convert_selection),
            ("convert_clipboard", &self.hotkeys.convert_clipboard),
            ("pause_toggle", &self.hotkeys.pause_toggle),
        ] {
            if value.trim().is_empty() {
                return Err(ConfigError::at(
                    text,
                    "hotkeys",
                    key,
                    &format!("{key} cannot be empty; give a binding such as \"Ctrl+Pause\""),
                ));
            }
        }

        Ok(())
    }
}

impl Exclusions {
    /// Whether keystrokes should never be recorded for this process.
    ///
    /// Accepts a bare executable name or a full path, and compares case-insensitively —
    /// Windows filenames are case-insensitive, and matching them exactly would silently fail
    /// to protect the applications this list exists for. Matching is whole-name, so a rule
    /// for `KeePassXC.exe` does not catch `KeePassXC.exe.bak`.
    pub fn is_excluded(&self, process: &str) -> bool {
        let name = process
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(process)
            .trim();
        self.processes
            .iter()
            .any(|listed| listed.trim().eq_ignore_ascii_case(name))
    }
}

/// A configuration that could not be used, and where the problem is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
    line: Option<usize>,
}

impl ConfigError {
    /// Build an error pointing at `key` within `[section]`.
    fn at(text: &str, section: &str, key: &str, message: &str) -> Self {
        Self {
            message: message.to_string(),
            line: locate(text, section, key),
        }
    }

    /// One-based line the problem is on, where that could be determined.
    pub fn line(&self) -> Option<usize> {
        self.line
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// One-based line containing a byte offset.
fn line_of_offset(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].lines().count().max(1)
}

/// Find the line where `key` is assigned inside `[section]`.
///
/// Scans rather than using spans because the value has already been deserialised by the
/// time it is validated. Returns the assignment inside the named section, so the same key
/// name in two sections does not point at the wrong one. `None` when the key is absent,
/// which happens when the value came from a default rather than the file — there is no line
/// to point at in that case, and the message alone has to do.
fn locate(text: &str, section: &str, key: &str) -> Option<usize> {
    let mut in_section = false;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_section = line.trim_start_matches('[').trim_end_matches(']').trim() == section;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((lhs, _)) = line.split_once('=') else {
            continue;
        };
        if lhs.trim() == key {
            return Some(i + 1);
        }
    }
    None
}
