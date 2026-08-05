//! Task 2.3 — position-based conversion.
//!
//! Conversion is not a translation between alphabets. Each character is resolved to the
//! physical key that produces it under the source layout, then to whatever that same key
//! produces under the target ([ADR-0004](../../docs/adr/0004-runtime-derived-layout-tables.md)).

mod common;

use lexiswitch_core::convert::convert;

#[test]
fn ukrainian_typed_under_an_english_layout() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // The canonical case: the keys that spell `features` on US English spell `ауфегкуі`
    // on Ukrainian, so this is what "convert what I actually typed" means.
    assert_eq!(convert("ауфегкуі", &uk, &en).as_deref(), Some("features"));
}

#[test]
fn conversion_round_trips() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    assert_eq!(convert("features", &en, &uk).as_deref(), Some("ауфегкуі"));
}

#[test]
fn mixed_case_is_preserved() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Case survives because shift is part of the position, not a separate concern: the
    // shifted key on one layout maps to the shifted key on the other.
    assert_eq!(convert("Ауфегкуі", &uk, &en).as_deref(), Some("Features"));
    assert_eq!(convert("АУФЕГКУІ", &uk, &en).as_deref(), Some("FEATURES"));
    assert_eq!(convert("аУфЕгКуІ", &uk, &en).as_deref(), Some("fEaTuReS"));
}

#[test]
fn qwertz_transposes_y_and_z() {
    let en = common::load("en_us");
    let de = common::load("de");
    // The key that types `y` on US types `z` on German, and vice versa. Nothing else in
    // the Latin alphabet moves between these two layouts.
    assert_eq!(convert("yz", &en, &de).as_deref(), Some("zy"));
    assert_eq!(convert("YZ", &en, &de).as_deref(), Some("ZY"));
    assert_eq!(convert("zy", &de, &en).as_deref(), Some("yz"));
}

#[test]
fn a_word_unaffected_by_the_transposition_still_converts_letter_for_letter() {
    let en = common::load("en_us");
    let de = common::load("de");
    // `cat` occupies the same positions on both layouts, so this must come back as a
    // no-change rather than a mangling — see task 2.7 for why that is `None`.
    assert_eq!(convert("cat", &en, &de), None);
}

#[test]
fn conversion_is_per_character_not_per_word() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Spaces are not in the probed set, so they pass through untouched while the letters
    // around them convert.
    assert_eq!(
        convert("ауфегкуі ауфегкуі", &uk, &en).as_deref(),
        Some("features features")
    );
}

#[test]
fn digits_and_shared_punctuation_convert_by_position() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Digits sit on the same keys in both layouts, so a digits-only string is a no-change.
    assert_eq!(convert("2026", &uk, &en), None);
}
