//! Writing one setting back into a configuration file without disturbing the rest of it.
//!
//! The file is the complete description of behaviour, so a shortcut changed through the
//! interface has to appear there — otherwise the file stops being the source of truth and a
//! reload would silently undo the change. But it is also a file the user writes, with their
//! comments and their spacing in it, and re-serialising the parsed document would quietly
//! rewrite all of that. So this is a surgical edit on the text, not a round trip.

use lexiswitch_core::config::rewrite_hotkey;

/// The ordinary case, and the property that matters most: everything the user wrote is still
/// there, byte for byte, except the one value.
#[test]
fn replaces_the_value_and_nothing_else() {
    let before = "\
# LexiSwitch configuration
#
# The shortcut that converts the current selection.
[hotkeys]
convert_selection = \"Shift+Pause\"   # trailing comment

[logging]
enabled = true
";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    assert!(after.contains("convert_selection = \"Ctrl+Alt+K\""));
    assert!(after.contains("# LexiSwitch configuration"));
    assert!(after.contains("# The shortcut that converts the current selection."));
    assert!(after.contains("[logging]"));
    assert!(after.contains("enabled = true"));
    assert!(!after.contains("Shift+Pause"));
}

/// A trailing comment on the line being changed belongs to the user too.
#[test]
fn keeps_a_trailing_comment_on_the_changed_line() {
    let before = "[hotkeys]\nconvert_selection = \"Shift+Pause\" # why this one\n";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    assert!(after.contains("# why this one"), "{after}");
}

/// The key may be missing from a table that exists.
#[test]
fn inserts_into_an_existing_table() {
    let before = "[hotkeys]\n\n[logging]\nenabled = false\n";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    assert!(after.contains("convert_selection = \"Ctrl+Alt+K\""));
    assert!(after.contains("[logging]"));
    assert!(after.contains("enabled = false"));
    // It must land inside [hotkeys], not after [logging].
    let hotkeys = after.find("[hotkeys]").unwrap();
    let logging = after.find("[logging]").unwrap();
    let key = after.find("convert_selection").unwrap();
    assert!(hotkeys < key && key < logging, "{after}");
}

/// The table may be missing entirely, including from an empty file.
#[test]
fn creates_the_table_when_absent() {
    for before in ["", "[logging]\nenabled = true\n"] {
        let after = rewrite_hotkey(before, "Ctrl+Alt+K");
        assert!(after.contains("[hotkeys]"), "{after}");
        assert!(after.contains("convert_selection = \"Ctrl+Alt+K\""), "{after}");
    }
}

/// A key of the same name in a different table is somebody else's setting.
#[test]
fn ignores_the_same_key_in_another_table() {
    let before = "\
[other]
convert_selection = \"Shift+Pause\"

[hotkeys]
convert_selection = \"Shift+Pause\"
";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    let other = after.find("[other]").unwrap();
    let hotkeys = after.find("[hotkeys]").unwrap();
    // The one before [hotkeys] is untouched; the one after it changed.
    assert!(after[other..hotkeys].contains("Shift+Pause"), "{after}");
    assert!(after[hotkeys..].contains("Ctrl+Alt+K"), "{after}");
    assert!(!after[hotkeys..].contains("Shift+Pause"), "{after}");
}

/// Windows files have CRLF endings and the user's editor will keep them.
#[test]
fn preserves_crlf_line_endings() {
    let before = "[hotkeys]\r\nconvert_selection = \"Shift+Pause\"\r\n\r\n[logging]\r\nenabled = true\r\n";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    assert!(after.contains("\r\n"), "line endings were rewritten");
    assert!(!after.contains("\n\n[logging]"), "an LF crept in: {after:?}");
    assert!(after.contains("convert_selection = \"Ctrl+Alt+K\""));
}

/// Whatever the user's spacing around the equals sign, the edit is to the value.
#[test]
fn tolerates_odd_spacing() {
    for before in [
        "[hotkeys]\nconvert_selection=\"Shift+Pause\"\n",
        "[hotkeys]\n  convert_selection   =   \"Shift+Pause\"\n",
        "[hotkeys]\nconvert_selection = 'Shift+Pause'\n",
    ] {
        let after = rewrite_hotkey(before, "Ctrl+Alt+K");
        assert!(after.contains("Ctrl+Alt+K"), "{before:?} -> {after:?}");
        assert!(!after.contains("Shift+Pause"), "{before:?} -> {after:?}");
    }
}

/// The result has to be loadable, or the interface would write a file the program then rejects.
#[test]
fn the_result_still_parses() {
    let before = "# a comment\n[hotkeys]\nconvert_selection = \"Shift+Pause\"\n\n[logging]\nenabled = true\n";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    let parsed: lexiswitch_core::config::Config =
        toml::from_str(&after).expect("the rewritten file must parse");
    assert_eq!(parsed.hotkeys.convert_selection, "Ctrl+Alt+K");
    assert!(parsed.logging.enabled);
}

/// A commented-out setting is not the setting. Editing it would leave the change invisible to
/// the parser and the user staring at a file that looks right.
#[test]
fn does_not_edit_a_commented_out_line() {
    let before = "[hotkeys]\n# convert_selection = \"Shift+Pause\"\n";
    let after = rewrite_hotkey(before, "Ctrl+Alt+K");
    assert!(after.contains("# convert_selection = \"Shift+Pause\""), "{after}");
    let parsed: lexiswitch_core::config::Config = toml::from_str(&after).unwrap();
    assert_eq!(parsed.hotkeys.convert_selection, "Ctrl+Alt+K");
}

/// One blank line before an appended table, not two, and not none — the file is one the user
/// reads, and however theirs happened to end should not change how ours looks.
#[test]
fn separates_an_appended_table_with_one_blank_line() {
    for before in [
        "[logging]\nenabled = true\n",
        "[logging]\nenabled = true",
        "[logging]\nenabled = true\n\n",
        // Line endings are not necessarily uniform. This is the shape of the real file on the
        // development machine, which already ends with a blank line — expressed with a mixture
        // of endings, so an exact byte comparison does not see it.
        "[logging]\nenabled = true\n\r\n",
    ] {
        let after = rewrite_hotkey(before, "Ctrl+Alt+K");
        // Counted rather than matched, for the same reason the implementation counts: the
        // separator may be any mixture of CR and LF, and only the number of line breaks
        // between the last setting and the new table is the thing under test.
        let gap = &after[after.find("true").unwrap() + 4..after.find("[hotkeys]").unwrap()];
        assert_eq!(
            gap.matches('\n').count(),
            2,
            "{before:?} -> gap {gap:?} in {after:?}"
        );
    }
}

/// Task 4.3: a setting changed through the interface appears in the file and survives a
/// reload — which is the whole reason the interface writes to the file at all.
#[test]
fn a_changed_shortcut_survives_a_reload() {
    let original = "# mine\n[hotkeys]\nconvert_selection = \"Shift+Pause\"\n\n[logging]\nenabled = true\n";
    let written = rewrite_hotkey(original, "Ctrl+Alt+K");
    let reloaded = lexiswitch_core::config::Config::parse(&written).expect("must reload");
    assert_eq!(reloaded.hotkeys.convert_selection, "Ctrl+Alt+K");
    assert!(reloaded.logging.enabled, "an unrelated setting was lost");
    assert!(written.contains("# mine"), "a comment was lost");
}

/// The same, for the enable state the tray toggle writes back.
#[test]
fn the_enabled_state_round_trips() {
    use lexiswitch_core::config::rewrite_setting;
    let written = rewrite_setting("[logging]\nenabled = true\n", "general", "enabled", "false");
    let reloaded = lexiswitch_core::config::Config::parse(&written).expect("must reload");
    assert!(!reloaded.general.enabled, "{written}");
    // `logging.enabled` has the same key name in a different table and must be untouched.
    assert!(reloaded.logging.enabled, "the wrong table was edited: {written}");

    let back = rewrite_setting(&written, "general", "enabled", "true");
    assert!(lexiswitch_core::config::Config::parse(&back).unwrap().general.enabled);
}

/// Task 4.4: an invalid configuration is rejected outright, so a caller holding a previous one
/// keeps it. Nothing partial is ever handed back.
#[test]
fn an_invalid_configuration_is_refused_whole() {
    // Valid TOML, invalid setting: the binding cannot be empty.
    let bad = "[hotkeys]\nconvert_selection = \"\"\n[logging]\nenabled = true\n";
    let err = lexiswitch_core::config::Config::parse(bad).expect_err("must be refused");
    assert!(err.line().is_some(), "an error without a location: {err}");

    // Not TOML at all.
    assert!(lexiswitch_core::config::Config::parse("[hotkeys\nbroken").is_err());
}
