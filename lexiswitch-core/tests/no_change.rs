//! Tasks 2.7 / 2.8 — "nothing would change" is an answer, not an empty result.
//!
//! The caller is a hotkey. Replacing a word with an identical word means deleting and
//! retyping text for no reason, in someone else's application, with a cursor to disturb —
//! so "this conversion is a no-op" has to be distinguishable from "here is your text".
//! `convert` returns `None`; `convert_lossy` still hands back the string for callers that
//! have already decided to act.

mod common;

use lexiswitch_core::convert::{convert, convert_lossy};

#[test]
fn an_identical_result_is_reported_as_no_change() {
    let en = common::load("en_us");
    let de = common::load("de");
    // Every character maps — c, a and t occupy the same positions on both layouts — and the
    // result is the input. That is a no-change, not a success.
    assert_eq!(convert("cat", &en, &de), None);
}

#[test]
fn the_identical_result_is_still_available_when_asked_for() {
    let en = common::load("en_us");
    let de = common::load("de");
    // The distinction 2.7 asks for: same input, same layouts, two different questions.
    assert_eq!(convert("cat", &en, &de), None);
    assert_eq!(convert_lossy("cat", &en, &de), "cat");
}

#[test]
fn a_real_conversion_is_not_reported_as_no_change() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    assert!(convert("ауфегкуі", &uk, &en).is_some());
}

#[test]
fn converting_a_layout_to_itself_is_always_a_no_change() {
    for name in ["uk", "en_us", "de"] {
        let table = common::load(name);
        assert_eq!(convert("ауфегкуі features xyz 123", &table, &table), None);
    }
}

#[test]
fn empty_text_is_a_no_change() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    assert_eq!(convert("", &uk, &en), None);
    assert_eq!(convert_lossy("", &uk, &en), "");
}

#[test]
fn one_changed_character_is_enough_to_be_a_change() {
    let en = common::load("en_us");
    let de = common::load("de");
    // `cat` alone is a no-change; adding a single y makes the whole string a conversion.
    assert_eq!(convert("cat", &en, &de), None);
    assert_eq!(convert("caty", &en, &de).as_deref(), Some("catz"));
}
