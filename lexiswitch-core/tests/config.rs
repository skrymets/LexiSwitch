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
    assert_eq!(config.hotkeys.convert_selection, "Win+Shift+Pause");
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
    // Both defaults share the Win+…+Pause shape deliberately: one gesture, one finger
    // moved. They are asserted together so neither can drift from the other.
    assert_eq!(c.hotkeys.convert_selection, "Win+Shift+Pause");
    assert_eq!(c.hotkeys.change_case, "Win+Ctrl+Pause");
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
    assert_eq!(config.hotkeys.convert_selection, "Win+Shift+Pause");
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

// --- the case-conversion shortcut ----------------------------------------------------
//
// A second transaction means a second binding (ADR-0014). It defaults like the first, so an
// existing configuration file written before this version keeps working and simply gains the
// default — which is what "settings that no longer exist are ignored" has to be mirrored by
// on the way in.

/// The default is the combination the design settled on, and it is present without being
/// written down.
#[test]
fn the_case_shortcut_has_a_default() {
    let config = Config::parse("").expect("an empty file is valid");
    assert_eq!(config.hotkeys.change_case, "Win+Ctrl+Pause");
}

/// A file from before this version has no `change_case` key. It must still load, and must not
/// report the missing key as a problem — it was never the user's to write.
#[test]
fn a_file_without_the_case_shortcut_still_loads() {
    let config = Config::parse(
        r#"
[hotkeys]
convert_selection = "Ctrl+Alt+K"
"#,
    )
    .expect("a file predating the case shortcut is valid");
    assert_eq!(config.hotkeys.convert_selection, "Ctrl+Alt+K");
    assert_eq!(config.hotkeys.change_case, "Win+Ctrl+Pause");
    assert!(
        config.ignored_keys().is_empty(),
        "{:?}",
        config.ignored_keys()
    );
}

/// Both bindings are read, and neither is confused for the other.
#[test]
fn both_shortcuts_are_read() {
    let config = Config::parse(
        r#"
[hotkeys]
convert_selection = "Ctrl+Alt+K"
change_case = "Ctrl+Alt+C"
"#,
    )
    .expect("valid");
    assert_eq!(config.hotkeys.convert_selection, "Ctrl+Alt+K");
    assert_eq!(config.hotkeys.change_case, "Ctrl+Alt+C");
}

/// An empty binding is refused for the same reason the first one is: it cannot be registered,
/// and silently defaulting it would make the file a description of something else.
#[test]
fn an_empty_case_shortcut_is_rejected_with_its_line() {
    let text = "[hotkeys]\nconvert_selection = \"Shift+Pause\"\nchange_case = \"\"\n";
    let err = Config::parse(text).expect_err("an empty binding is not usable");
    assert!(err.to_string().contains("change_case"), "got {err}");
    assert_eq!(err.line(), Some(3), "the offending line is not pointed at");
}

/// The two must differ. One combination cannot open two windows, and the failure would
/// otherwise be a registration conflict the user reads as another application's fault.
#[test]
fn the_two_shortcuts_may_not_be_the_same() {
    let text = "[hotkeys]\nconvert_selection = \"Shift+Pause\"\nchange_case = \"Shift+Pause\"\n";
    let err = Config::parse(text).expect_err("one combination cannot do both");
    assert!(err.to_string().contains("change_case"), "got {err}");
}

/// Differing only in spacing or case is still the same combination to Windows.
#[test]
fn the_sameness_check_is_not_fooled_by_spelling() {
    let text = "[hotkeys]\nconvert_selection = \"Shift+Pause\"\nchange_case = \" shift + pause \"\n";
    assert!(
        Config::parse(text).is_err(),
        "the same combination spelled differently was accepted"
    );
}

// --- the transliteration shortcut ----------------------------------------------------

#[test]
fn the_transliteration_shortcut_has_a_default() {
    let config = Config::parse("").expect("an empty file is valid");
    assert_eq!(config.hotkeys.transliterate, "Win+Alt+Pause");
    assert!(config.unbound().is_empty());
}

#[test]
fn an_empty_transliteration_shortcut_is_rejected_with_its_line() {
    let text = "[hotkeys]\ntransliterate = \"\"\n";
    let err = Config::parse(text).expect_err("an empty binding is not usable");
    assert!(err.to_string().contains("transliterate"), "got {err}");
    assert_eq!(err.line(), Some(2));
}

/// Two bindings the user wrote that collide are a mistake in the file, as before.
#[test]
fn a_written_transliteration_shortcut_may_not_repeat_another() {
    let text = "[hotkeys]\nconvert_selection = \"Ctrl+Alt+K\"\nchange_case = \"Ctrl+Alt+C\"\ntransliterate = \"Ctrl+Alt+C\"\n";
    let err = Config::parse(text).expect_err("one combination cannot do both");
    assert!(err.to_string().contains("transliterate"), "got {err}");
    assert!(err.to_string().contains("change_case"), "got {err}");
    assert_eq!(err.line(), Some(4));
}

/// A file from before this version that rebound case to the new default. The user's binding
/// wins; transliteration is left without a shortcut and says why. Rejecting the file would stop
/// the program starting after an upgrade.
#[test]
fn a_defaulted_shortcut_that_collides_with_a_written_one_is_left_unbound() {
    // Exactly the shape of the configuration this was designed against.
    let text = "[hotkeys]\nconvert_selection = \"Win+Shift+Pause\"\nchange_case = \"Alt+Win+Pause\"\n";
    let config = Config::parse(text).expect("an upgrade must not stop the program starting");
    assert_eq!(config.hotkeys.change_case, "Alt+Win+Pause");
    let unbound = config.unbound();
    assert_eq!(unbound.len(), 1);
    assert_eq!(unbound[0].key, "transliterate");
    assert_eq!(unbound[0].binding, "Win+Alt+Pause");
    assert_eq!(unbound[0].taken_by, "change_case");
}

/// The rule is about which binding was written, not about which shortcut is newest.
#[test]
fn any_defaulted_shortcut_gives_way_to_a_written_one() {
    let text = "[hotkeys]\ntransliterate = \"Win+Shift+Pause\"\n";
    let config = Config::parse(text).expect("valid");
    let unbound = config.unbound();
    assert_eq!(unbound.len(), 1);
    assert_eq!(unbound[0].key, "convert_selection");
    assert_eq!(unbound[0].taken_by, "transliterate");
}

#[test]
fn a_file_without_the_transliteration_shortcut_still_loads() {
    let config = Config::parse(PREVIOUS_VERSION_CONFIG).expect("valid");
    assert_eq!(config.hotkeys.transliterate, "Win+Alt+Pause");
    assert!(config.unbound().is_empty());
}
