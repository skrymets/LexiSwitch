//! Task 2.5 — pass-through.
//!
//! Three things cannot be converted, and all three leave the character alone rather than
//! dropping or substituting it: a character the source layout cannot produce, a position
//! never probed on the target, and a position that is a dead key on the target.
//!
//! Silently dropping would corrupt text. Substituting would invent one.

mod common;

use lexiswitch_core::convert::{convert, convert_lossy};

#[test]
fn a_position_that_is_dead_on_the_target_passes_through() {
    let en = common::load("en_us");
    let de = common::load("de");
    // `=` sits at 0x0d on US. That position is the dead ´/` key on German, so there is no
    // character to convert to. The `y -> z` beside it proves the rest still converts.
    assert_eq!(convert_lossy("=", &en, &de), "=");
    assert_eq!(convert("y=", &en, &de).as_deref(), Some("z="));
}

#[test]
fn every_dead_target_position_passes_through() {
    let en = common::load("en_us");
    let de = common::load("de");
    // 0x29 (US backtick) is German's dead ^, and 0x0d shifted is dead too.
    assert_eq!(convert_lossy("`", &en, &de), "`");
    assert_eq!(convert_lossy("+", &en, &de), "+");
}

#[test]
fn a_character_the_source_layout_cannot_produce_passes_through() {
    let en = common::load("en_us");
    let de = common::load("de");
    // US English has no ö, so there is no position to start from.
    assert_eq!(convert_lossy("ö", &en, &de), "ö");
    assert_eq!(convert_lossy("є", &en, &de), "є");
}

#[test]
fn characters_outside_every_layout_pass_through() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Whitespace, emoji, and anything else never probed.
    assert_eq!(convert_lossy(" \t\n", &uk, &en), " \t\n");
    assert_eq!(convert_lossy("🙂", &uk, &en), "🙂");
}

#[test]
fn altgr_characters_pass_through_because_they_were_never_probed() {
    let de = common::load("de");
    let en = common::load("en_us");
    // A documented gap, not a bug: the fixtures cover unshifted and shifted only, so
    // German's AltGr characters have no position. They survive unchanged, which is the
    // right failure — see docs/spikes/2026-08-05-windows-input-primitives.md.
    assert_eq!(convert_lossy("@", &de, &en), "@");
    assert_eq!(convert_lossy("€", &de, &en), "€");
}

#[test]
fn pass_through_is_mixed_with_conversion_character_by_character() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    // Convertible and unconvertible characters interleaved: each is decided alone.
    assert_eq!(
        convert("ауфегкуі — 🙂 ауфегкуі!", &uk, &en).as_deref(),
        Some("features — 🙂 features!")
    );
}

#[test]
fn a_string_of_only_unconvertible_characters_reports_no_change() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    assert_eq!(convert("🙂🙂🙂", &uk, &en), None);
}

#[test]
fn dead_source_positions_produce_no_character_to_convert() {
    let de = common::load("de");
    let en = common::load("en_us");
    // `^` and `´` are what German's dead keys are *labelled*, but typing them produces no
    // character, so neither is in the layout's reverse map and neither can be a conversion
    // input. They pass through as ordinary text.
    assert_eq!(de.position_of('^'), None);
    assert_eq!(convert_lossy("^", &de, &en), "^");
}
