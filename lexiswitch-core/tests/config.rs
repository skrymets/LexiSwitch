//! Tasks 4.1 / 4.3 / 4.5 — configuration parsing, defaults, validation, exclusions.

use lexiswitch_core::config::Config;

/// The shipped example, which must parse and must equal the defaults — otherwise the file
/// documents settings the program does not actually start with.
fn example() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../config.example.toml");
    std::fs::read_to_string(path).expect("config.example.toml at the repo root")
}

#[test]
fn the_shipped_example_parses() {
    let config = Config::parse(&example()).expect("example config must be valid");
    assert_eq!(config.hotkeys.convert_last_word, "Pause");
}

#[test]
fn the_shipped_example_is_exactly_the_defaults() {
    // If these diverge, the example file is lying about what the program does when the
    // file is absent.
    let from_file = Config::parse(&example()).unwrap();
    let defaults = Config::default();
    assert_eq!(from_file.hotkeys, defaults.hotkeys);
    assert_eq!(from_file.buffer, defaults.buffer);
    assert_eq!(from_file.selection, defaults.selection);
    assert_eq!(from_file.behavior, defaults.behavior);
    assert_eq!(from_file.logging, defaults.logging);
    assert_eq!(from_file.layouts, defaults.layouts);
    assert_eq!(from_file.exclusions, defaults.exclusions);
}

#[test]
fn an_empty_file_yields_the_defaults() {
    // A missing or empty config must behave exactly like the shipped example.
    assert_eq!(Config::parse("").unwrap(), Config::default());
}

#[test]
fn every_default_value_is_what_the_design_specifies() {
    let c = Config::default();
    assert_eq!(c.hotkeys.convert_last_word, "Pause");
    assert_eq!(c.hotkeys.convert_selection, "Shift+Pause");
    assert_eq!(c.hotkeys.convert_clipboard, "Ctrl+Pause");
    assert_eq!(c.hotkeys.pause_toggle, "Ctrl+Shift+Pause");
    assert_eq!(c.hotkeys.multi_tap_window_ms, 500);
    assert!(c.layouts.candidates.is_empty());
    assert_eq!(c.buffer.idle_timeout_secs, 30);
    assert!(c.buffer.respect_password_fields);
    assert!(c.selection.allow_clipboard_fallback);
    // Off by default: switching after a correction changes what the *next* word will
    // produce, which surprises more often than it helps.
    assert!(!c.behavior.switch_layout_after_correction);
    assert!(!c.logging.enabled);
}

#[test]
fn a_partial_file_fills_the_rest_from_defaults() {
    let config = Config::parse("[buffer]\nidle_timeout_secs = 5\n").unwrap();
    assert_eq!(config.buffer.idle_timeout_secs, 5);
    // Untouched sections are still the defaults.
    assert_eq!(config.hotkeys.convert_last_word, "Pause");
    assert!(config.buffer.respect_password_fields);
}

// --- 4.3: validation refuses to disable the password-field protection -------------------

#[test]
fn disabling_password_field_protection_is_rejected() {
    let err = Config::parse("[buffer]\nrespect_password_fields = false\n")
        .expect_err("must be refused, not honoured");
    assert!(
        err.to_string().contains("respect_password_fields"),
        "the error must name the setting: {err}"
    );
}

#[test]
fn the_rejection_names_the_line_it_is_on() {
    let text = "\
# a comment
[buffer]
idle_timeout_secs = 30
respect_password_fields = false
";
    let err = Config::parse(text).expect_err("must be refused");
    assert_eq!(err.line(), Some(4), "wrong line in: {err}");
}

#[test]
fn setting_password_field_protection_to_true_is_fine() {
    let config = Config::parse("[buffer]\nrespect_password_fields = true\n").unwrap();
    assert!(config.buffer.respect_password_fields);
}

#[test]
fn malformed_toml_is_reported_with_a_line() {
    let err = Config::parse("[buffer\nidle_timeout_secs = 30\n").expect_err("malformed");
    assert!(err.line().is_some(), "syntax errors must carry a line: {err}");
}

#[test]
fn out_of_range_values_are_rejected_with_their_line() {
    // A multi-tap window below the 15.625 ms timestamp granularity cannot be honoured.
    let err = Config::parse("[hotkeys]\nmulti_tap_window_ms = 3\n").expect_err("too small");
    assert_eq!(err.line(), Some(2));

    let err = Config::parse("[buffer]\nidle_timeout_secs = 0\n").expect_err("zero timeout");
    assert_eq!(err.line(), Some(2));
}

#[test]
fn an_empty_hotkey_binding_is_rejected() {
    let err = Config::parse("[hotkeys]\nconvert_last_word = \"\"\n").expect_err("empty binding");
    assert!(err.to_string().contains("convert_last_word"), "{err}");
}

#[test]
fn an_unknown_key_is_rejected_rather_than_ignored() {
    // A typo that is silently ignored looks exactly like a setting that does not work.
    let err = Config::parse("[buffer]\nidle_timeout_seconds = 30\n").expect_err("typo");
    assert!(err.to_string().contains("idle_timeout_seconds"), "{err}");
}

// --- 4.5: exclusion matching by process name -------------------------------------------

#[test]
fn a_listed_process_is_excluded() {
    let c = Config::default();
    assert!(c.exclusions.is_excluded("KeePassXC.exe"));
    assert!(!c.exclusions.is_excluded("notepad.exe"));
}

#[test]
fn windows_terminal_is_excluded_by_default() {
    // Not a preference. A terminal password prompt has no IsPassword property to detect —
    // it is a shell that stopped echoing — so exclusion is the only containment available.
    // Measured in docs/manual-tests.md; removing this re-opens a gap that detection cannot
    // close.
    let c = Config::default();
    assert!(c.exclusions.is_excluded("WindowsTerminal.exe"));
}

#[test]
fn exclusion_matching_ignores_case() {
    // Windows executable names are case-insensitive; matching them case-sensitively would
    // silently fail to protect the very applications the list exists for.
    let c = Config::default();
    assert!(c.exclusions.is_excluded("keepassxc.exe"));
    assert!(c.exclusions.is_excluded("KEEPASSXC.EXE"));
}

#[test]
fn exclusion_matching_accepts_a_full_path() {
    let c = Config::default();
    assert!(c.exclusions.is_excluded(r"C:\Program Files\KeePassXC\KeePassXC.exe"));
    assert!(!c.exclusions.is_excluded(r"C:\Windows\System32\notepad.exe"));
}

#[test]
fn a_partial_name_does_not_match() {
    // "Pass.exe" must not be caught by a rule for "KeePassXC.exe", and vice versa.
    let c = Config::parse("[exclusions]\nprocesses = [\"KeePassXC.exe\"]\n").unwrap();
    assert!(!c.exclusions.is_excluded("Pass.exe"));
    assert!(!c.exclusions.is_excluded("KeePassXC.exe.bak"));
    assert!(!c.exclusions.is_excluded("XKeePassXC.exe"));
}

#[test]
fn an_empty_exclusion_list_excludes_nothing() {
    let c = Config::parse("[exclusions]\nprocesses = []\n").unwrap();
    assert!(!c.exclusions.is_excluded("KeePassXC.exe"));
}
