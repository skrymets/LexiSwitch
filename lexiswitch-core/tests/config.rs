//! Configuration parsing, defaults, validation, and migration from an earlier version.

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
    assert_eq!(config.hotkeys.convert_selection, "Shift+Pause");
}

#[test]
fn the_shipped_example_is_exactly_the_defaults() {
    // If these diverge, the example file is lying about what the program does when the
    // file is absent.
    let from_file = Config::parse(&example()).unwrap();
    let defaults = Config::default();
    assert_eq!(from_file.hotkeys, defaults.hotkeys);
    assert_eq!(from_file.selection, defaults.selection);
    assert_eq!(from_file.logging, defaults.logging);
    assert_eq!(from_file.layouts, defaults.layouts);
}

#[test]
fn an_empty_file_yields_the_defaults() {
    assert_eq!(Config::parse("").unwrap(), Config::default());
}

#[test]
fn every_default_value_is_what_the_design_specifies() {
    let c = Config::default();
    assert_eq!(c.hotkeys.convert_selection, "Shift+Pause");
    assert!(c.selection.allow_clipboard_fallback);
    assert!(!c.logging.enabled, "logging is off unless asked for");
    assert!(c.layouts.candidates.is_empty());
}

#[test]
fn a_partial_file_fills_the_rest_from_defaults() {
    let config = Config::parse("[logging]
enabled = true
").unwrap();
    assert!(config.logging.enabled);
    // Untouched sections still carry their defaults.
    assert_eq!(config.hotkeys.convert_selection, "Shift+Pause");
    assert!(config.selection.allow_clipboard_fallback);
}

#[test]
fn malformed_toml_is_reported_with_a_line() {
    let err = Config::parse("[logging
enabled = true
").expect_err("malformed");
    assert!(err.to_string().contains("line"), "got {err}");
}

#[test]
fn an_empty_hotkey_binding_is_rejected() {
    // Structurally valid TOML, semantically useless: a binding that cannot be registered.
    let err = Config::parse("[hotkeys]
convert_selection = \"\"
").expect_err("empty binding");
    assert!(err.to_string().contains("convert_selection"), "got {err}");
}

// --- migration from an earlier version -------------------------------------------------
//
// The settings below were removed when keystroke capture was retired. A file still carrying
// them must not stop the program from starting: validation failure means "keep last known
// good", and on the first run after an upgrade there is no last known good to keep.

/// Exactly what the previous version shipped, so this is the real upgrade path rather than a
/// synthetic one.
const PREVIOUS_VERSION_CONFIG: &str = r#"
[hotkeys]
convert_last_word = "Pause"
convert_selection = "Shift+Pause"
convert_clipboard = "Ctrl+Pause"
pause_toggle = "Ctrl+Shift+Pause"
multi_tap_window_ms = 500

[buffer]
idle_timeout_secs = 30
respect_password_fields = true

[selection]
allow_clipboard_fallback = true

[exclusions]
processes = ["KeePassXC.exe"]

[behavior]
switch_layout_after_correction = false

[logging]
enabled = false
"#;

#[test]
fn a_config_from_the_previous_version_still_loads() {
    let config = Config::parse(PREVIOUS_VERSION_CONFIG)
        .expect("a config file from the previous version must not stop the program starting");
    assert_eq!(config.hotkeys.convert_selection, "Shift+Pause");
    assert!(config.selection.allow_clipboard_fallback);
}

#[test]
fn removed_settings_are_reported_by_name() {
    let config = Config::parse(PREVIOUS_VERSION_CONFIG).unwrap();
    let ignored = config.ignored_keys();

    // Whole sections that no longer exist, and keys inside sections that still do.
    for expected in [
        "buffer",
        "exclusions",
        "behavior",
        "hotkeys.convert_last_word",
        "hotkeys.convert_clipboard",
        "hotkeys.pause_toggle",
        "hotkeys.multi_tap_window_ms",
    ] {
        assert!(
            ignored.iter().any(|k| k == expected),
            "expected {expected:?} to be reported as ignored, got {ignored:?}"
        );
    }
}

#[test]
fn a_misspelled_key_is_reported_rather_than_silently_dropped() {
    // The reason ignoring unknown keys is not enough on its own: a typo and a removed
    // setting look identical from the outside unless the program says which it saw.
    let config = Config::parse("[hotkeys]\nconvert_selektion = \"Shift+Pause\"\n").unwrap();
    assert_eq!(config.ignored_keys(), vec!["hotkeys.convert_selektion"]);
}

#[test]
fn the_shipped_example_has_nothing_to_ignore() {
    // Catches drift the other way: the example file documenting a setting the program no
    // longer has.
    let config = Config::parse(&example()).unwrap();
    assert_eq!(config.ignored_keys(), Vec::<String>::new());
}
