//! Task 2.10 — regression corpus.
//!
//! Cases that are easy to break and easy not to notice: characters that only exist on one
//! layout, punctuation that moves under shift, digits that mostly do not move at all, and
//! strings mixing scripts. Each expectation was computed from the frozen fixtures, so a
//! failure here means conversion changed, not that the keyboard did.

mod common;

use lexiswitch_core::convert::{convert, convert_lossy};

#[test]
fn german_umlauts_convert_to_the_us_punctuation_sharing_their_keys() {
    let de = common::load("de");
    let en = common::load("en_us");
    // ö ä ü sit on the US ; ' [ keys.
    assert_eq!(convert_lossy("ö", &de, &en), ";");
    assert_eq!(convert_lossy("ä", &de, &en), "'");
    assert_eq!(convert_lossy("ü", &de, &en), "[");
    assert_eq!(convert_lossy("Ö", &de, &en), ":");
    assert_eq!(convert_lossy("Ä", &de, &en), "\"");
    assert_eq!(convert_lossy("Ü", &de, &en), "{");
    // And back the other way.
    assert_eq!(convert_lossy(";'[", &en, &de), "öäü");
}

#[test]
fn eszett_converts_and_returns() {
    let de = common::load("de");
    let en = common::load("en_us");
    // ß is on the US minus key.
    assert_eq!(convert_lossy("ß", &de, &en), "-");
    assert_eq!(convert_lossy("-", &en, &de), "ß");
}

#[test]
fn shifted_punctuation_moves_between_us_and_german() {
    let en = common::load("en_us");
    let de = common::load("de");
    // The number row diverges heavily under shift.
    assert_eq!(convert_lossy("@", &en, &de), "\"");
    assert_eq!(convert_lossy("#", &en, &de), "§");
    assert_eq!(convert_lossy("^", &en, &de), "&");
    assert_eq!(convert_lossy("&", &en, &de), "/");
    assert_eq!(convert_lossy("*", &en, &de), "(");
    assert_eq!(convert_lossy("(", &en, &de), ")");
    assert_eq!(convert_lossy(")", &en, &de), "=");
}

#[test]
fn digits_are_shared_by_all_three_layouts() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let de = common::load("de");
    // The unshifted number row is identical everywhere, so digits are always a no-change.
    for (a, b) in [(&uk, &en), (&en, &de), (&uk, &de), (&de, &uk)] {
        assert_eq!(convert("0123456789", a, b), None);
    }
}

#[test]
fn mixed_script_strings_convert_only_what_they_can() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Latin letters are not on the Ukrainian layout, so they pass through untouched while
    // the Cyrillic converts around them.
    assert_eq!(
        convert("ауфегкуі and features", &uk, &en).as_deref(),
        Some("features and features")
    );
}

#[test]
fn ukrainian_specific_letters_survive_a_trip_through_english() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // і ї є are the letters that distinguish Ukrainian from Russian; they must map, not
    // pass through.
    assert_eq!(convert_lossy("і", &uk, &en), "s");
    assert_eq!(convert_lossy("ї", &uk, &en), "]");
    assert_eq!(convert_lossy("є", &uk, &en), "'");
    assert_eq!(convert_lossy("s]'", &en, &uk), "іїє");
}

#[test]
fn qwertz_transposition_survives_in_a_realistic_sentence() {
    let en = common::load("en_us");
    let de = common::load("de");
    // Every y and z swaps, in both directions, and nothing else in this sentence moves:
    // lazy -> layz, zebra -> yebra.
    assert_eq!(convert("lazy zebra", &en, &de).as_deref(), Some("layz yebra"));
    assert_eq!(convert("lazy", &en, &de).as_deref(), Some("layz"));
    // And the same sentence converted back is the original.
    assert_eq!(convert("layz yebra", &de, &en).as_deref(), Some("lazy zebra"));
}

#[test]
fn case_survives_the_full_alphabet() {
    let en = common::load("en_us");
    let de = common::load("de");
    let upper = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let converted = convert_lossy(upper, &en, &de);
    // Only Y and Z move, and both stay upper case.
    assert_eq!(converted, "ABCDEFGHIJKLMNOPQRSTUVWXZY");
    assert!(converted.chars().all(|c| c.is_uppercase()));
}

#[test]
fn text_with_no_convertible_characters_is_reported_as_no_change() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    assert_eq!(convert("   \t\n", &uk, &en), None);
    assert_eq!(convert("🙂🎹", &uk, &en), None);
}
