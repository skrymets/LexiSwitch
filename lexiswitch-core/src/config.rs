//! Configuration: types, defaults, and validation.
//!
//! A configuration is parsed and validated once, then treated as immutable. A reload builds
//! a whole new one and swaps it wholesale, so nothing ever reads half-old, half-new state —
//! and a file that fails validation simply does not get swapped in, which is how
//! last-known-good works without a rollback path.
//!
//! Unknown keys are **collected and reported**, not rejected and not silently dropped.
//! Rejecting turns an upgrade into a failure to start, because a file written by an earlier
//! version fails validation and validation failure means "keep the last known good" — which,
//! on the first run after an upgrade, does not exist. Dropping silently is friendlier but
//! destroys the distinction between a setting that was removed and one that was misspelled,
//! which is exactly what a user needs when a setting appears to do nothing.
//!
//! So every struct keeps what it did not recognise, and [`Config::ignored_keys`] names them.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Keys present in the file that no field claimed.
///
/// `toml::Value` carries floats, so these types cannot derive `Eq`; `PartialEq` is all the
/// program compares with.
type Unknown = BTreeMap<String, toml::Value>;

/// The whole configurable surface, in one readable place.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub hotkeys: Hotkeys,
    pub layouts: Layouts,
    pub selection: Selection,
    pub chooser: Chooser,
    pub logging: Logging,
    #[serde(flatten)]
    unknown: Unknown,
}

/// Settings with nowhere more specific to live.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct General {
    /// Whether the shortcut converts. Written back when the tray toggle is used, so the choice
    /// survives a restart — a setting the user changes through the interface has to appear in
    /// the file, or the file stops being the complete description of behaviour.
    pub enabled: bool,
    #[serde(flatten)]
    unknown: Unknown,
}

/// How the chooser presents candidates.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Chooser {
    /// How many wrapped lines the expanded view shows before it scrolls. Clamped when read,
    /// because a value of zero would produce a window with no room for the text it exists to
    /// show, and a very large one a window taller than the screen.
    pub expanded_lines: u32,
    #[serde(flatten)]
    unknown: Unknown,
}

impl Chooser {
    /// The setting, held to a range that can actually be drawn.
    pub fn expanded_lines(&self) -> i32 {
        self.expanded_lines.clamp(3, 40) as i32
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub convert_selection: String,
    /// Opens the case window. A separate transaction, so a separate binding
    /// ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)).
    pub change_case: String,
    #[serde(flatten)]
    unknown: Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Layouts {
    /// Cycle order by language tag. Empty means installed order, layout active at typing
    /// time first.
    pub candidates: Vec<String>,
    #[serde(flatten)]
    unknown: Unknown,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub allow_clipboard_fallback: bool,
    #[serde(flatten)]
    unknown: Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Logging {
    /// Off unless explicitly enabled, which is also the derived default — the two must not
    /// drift apart, so this relies on the derive rather than restating it.
    pub enabled: bool,
    #[serde(flatten)]
    unknown: Unknown,
}

impl Default for General {
    fn default() -> Self {
        Self {
            enabled: true,
            unknown: Unknown::new(),
        }
    }
}

impl Default for Chooser {
    fn default() -> Self {
        Self {
            expanded_lines: 10,
            unknown: Unknown::new(),
        }
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            convert_selection: "Shift+Pause".into(),
            // Next to the first on the same physical key, so the pair is learned as one
            // gesture rather than two unrelated combinations.
            change_case: "Win+Ctrl+Pause".into(),
            unknown: Unknown::new(),
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            allow_clipboard_fallback: true,
            unknown: Unknown::new(),
        }
    }
}

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

    /// Keys the file carried that nothing claimed, as dotted paths, sorted.
    ///
    /// Both a removed setting and a misspelled one land here, which is the point: the caller
    /// names them so the user can tell which they are looking at. Reporting is all that
    /// happens — an unrecognised key never stops the program starting.
    pub fn ignored_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.unknown.keys().cloned().collect();
        let sections: [(&str, &Unknown); 6] = [
            ("general", &self.general.unknown),
            ("chooser", &self.chooser.unknown),
            ("hotkeys", &self.hotkeys.unknown),
            ("layouts", &self.layouts.unknown),
            ("selection", &self.selection.unknown),
            ("logging", &self.logging.unknown),
        ];
        for (section, unknown) in sections {
            keys.extend(unknown.keys().map(|k| format!("{section}.{k}")));
        }
        keys.sort();
        keys
    }

    /// Rules that TOML itself cannot express.
    ///
    /// Each failure carries the line the offending setting sits on, because "your config is
    /// wrong" without a location is a worse experience than no message at all.
    fn validate(&self, text: &str) -> Result<(), ConfigError> {
        if self.hotkeys.convert_selection.trim().is_empty() {
            return Err(ConfigError::at(
                text,
                "hotkeys",
                "convert_selection",
                "convert_selection cannot be empty; give a binding such as \"Shift+Pause\"",
            ));
        }
        if self.hotkeys.change_case.trim().is_empty() {
            return Err(ConfigError::at(
                text,
                "hotkeys",
                "change_case",
                "change_case cannot be empty; give a binding such as \"Win+Ctrl+Pause\"",
            ));
        }
        // Caught here rather than at registration, where it would surface as
        // ERROR_HOTKEY_ALREADY_REGISTERED and read as another application's fault. Compared
        // loosely because "Shift+Pause" and " shift + pause " are the same combination to
        // Windows and differ only in how they were typed into the file.
        if same_binding(
            &self.hotkeys.convert_selection,
            &self.hotkeys.change_case,
        ) {
            return Err(ConfigError::at(
                text,
                "hotkeys",
                "change_case",
                "change_case and convert_selection cannot be the same combination; one \
                 combination cannot open two different windows",
            ));
        }

        Ok(())
    }
}

/// Whether two written bindings name the same combination.
///
/// Spelling is not the question: `RegisterHotKey` sees modifiers and a virtual key, so
/// `"Shift+Pause"` and `" shift + pause "` are one combination written two ways. Compared
/// component by component rather than as whole strings, because `"Ctrl+Shift+K"` and
/// `"Shift+Ctrl+K"` are also the same and a trimmed case-insensitive string compare would miss
/// it. Order within the combination is the user's, not a fact about the binding.
fn same_binding(a: &str, b: &str) -> bool {
    fn parts(text: &str) -> Vec<String> {
        let mut v: Vec<String> = text
            .split('+')
            .map(|p| p.trim().to_ascii_lowercase())
            .filter(|p| !p.is_empty())
            .collect();
        v.sort();
        v
    }
    let (a, b) = (parts(a), parts(b));
    !a.is_empty() && a == b
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

/// Write `binding` into the `convert_selection` setting of a configuration file's text.
///
/// A surgical edit, not a round trip. Parsing the file and re-serialising it would produce a
/// valid document and destroy everything about it that is the user's: their comments, their
/// spacing, the order they chose to put things in. The file is the complete description of
/// behaviour, so the interface has to write to it — and it is the user's file, so it has to
/// write as little as possible.
///
/// Handles the three shapes the file can be in: the setting is present, the `[hotkeys]` table
/// is present without it, or neither is. Line endings are taken from the file rather than
/// assumed, because a Windows editor will have left CRLF and rewriting them would show up as a
/// whole-file diff.
///
/// A commented-out setting is deliberately not treated as the setting. Editing it would leave
/// the change invisible to the parser while looking correct in the editor.
pub fn rewrite_hotkey(text: &str, binding: &str) -> String {
    rewrite_setting(
        text,
        "hotkeys",
        "convert_selection",
        &format!("\"{}\"", binding.replace('"', "\\\"")),
    )
}

/// Write `value` into `table.key`, leaving every other byte of the file alone.
///
/// `value` is already TOML: quoted for a string, bare for a boolean or a number. The caller
/// knows the type and this function deliberately does not — guessing would mean quoting `true`
/// or failing to quote a binding that happens to look numeric.
pub fn rewrite_setting(text: &str, table: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let setting = format!("{key} = {value}");
    let header = format!("[{table}]");

    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let mut in_hotkeys = false;
    let mut hotkeys_end: Option<usize> = None;
    let mut seen_hotkeys = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            // Leaving `[hotkeys]`: this is where the setting goes if it was never in it.
            if in_hotkeys {
                hotkeys_end = Some(i);
            }
            in_hotkeys = trimmed.starts_with(&header);
            seen_hotkeys |= in_hotkeys;
            continue;
        }
        if !in_hotkeys || trimmed.starts_with('#') {
            continue;
        }
        // The key, with whatever spacing surrounds it before the `=`.
        let Some((found, _)) = line.split_once('=') else {
            continue;
        };
        if found.trim() != key {
            continue;
        }
        // Keep any trailing comment: it is the user's note about this very setting.
        let comment = line
            .find('#')
            .map(|at| format!(" {}", line[at..].trim_end_matches(['\r', '\n'])))
            .unwrap_or_default();
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        lines[i] = format!("{indent}{setting}{comment}{newline}");
        return lines.concat();
    }

    // Not found. Either the table exists and the setting does not, or neither does.
    let mut out = lines.concat();
    match hotkeys_end.or(if in_hotkeys { Some(lines.len()) } else { None }) {
        Some(at) => {
            lines.insert(at, format!("{setting}{newline}"));
            lines.concat()
        }
        None => {
            debug_assert!(!seen_hotkeys, "the table was found but not its extent");
            // Exactly one blank line before the new table, however the file happened to end.
            // The user reads this file; how theirs was terminated should not change how ours
            // looks.
            if !out.is_empty() {
                // Counted, not matched. A file can end with a blank line and still not end with
                // the two-newline sequence being looked for, because line endings are not
                // necessarily uniform: the real configuration file on the development machine
                // ends `\n\r\n`, so a byte comparison against `\r\n\r\n` missed it and added a
                // second blank line every time it was rewritten.
                let content = out.trim_end_matches(['\r', '\n']);
                let breaks = out[content.len()..].matches('\n').count();
                for _ in breaks..2 {
                    out.push_str(newline);
                }
            }
            out.push_str(&format!("{header}{newline}{setting}{newline}"));
            out
        }
    }
}
