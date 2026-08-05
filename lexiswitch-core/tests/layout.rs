//! Task 2.1 — loading a `LayoutTable` from fixture data, including dead-key positions.

mod common;

use lexiswitch_core::layout::{KeyPos, KeyValue};

#[test]
fn all_three_fixtures_load() {
    for name in ["uk", "en_us", "de"] {
        let table = common::load(name);
        assert_eq!(table.name(), name);
        assert!(
            table.len() > 80,
            "{name} has only {} positions; the probed set is 48 keys in 2 shift states",
            table.len()
        );
    }
}

#[test]
fn positions_resolve_to_the_character_the_key_produces() {
    let en = common::load("en_us");
    assert_eq!(en.value_at(KeyPos::new(0x15, false)), Some(KeyValue::Char('y')));
    assert_eq!(en.value_at(KeyPos::new(0x1e, false)), Some(KeyValue::Char('a')));
    assert_eq!(en.value_at(KeyPos::new(0x1e, true)), Some(KeyValue::Char('A')));
}

#[test]
fn german_is_qwertz_not_qwerty() {
    let de = common::load("de");
    // The y/z transposition, which is the whole reason German is in the fixture set
    // alongside its dead keys.
    assert_eq!(de.value_at(KeyPos::new(0x15, false)), Some(KeyValue::Char('z')));
    assert_eq!(de.value_at(KeyPos::new(0x2c, false)), Some(KeyValue::Char('y')));
    // Umlauts sit where US punctuation does.
    assert_eq!(de.value_at(KeyPos::new(0x27, false)), Some(KeyValue::Char('ö')));
    assert_eq!(de.value_at(KeyPos::new(0x28, false)), Some(KeyValue::Char('ä')));
    assert_eq!(de.value_at(KeyPos::new(0x1a, false)), Some(KeyValue::Char('ü')));
}

#[test]
fn ukrainian_letters_load_including_the_iso_extra_key() {
    let uk = common::load("uk");
    assert_eq!(uk.value_at(KeyPos::new(0x21, false)), Some(KeyValue::Char('а')));
    assert_eq!(uk.value_at(KeyPos::new(0x28, false)), Some(KeyValue::Char('є')));
    assert_eq!(uk.value_at(KeyPos::new(0x1b, false)), Some(KeyValue::Char('ї')));
    assert_eq!(uk.value_at(KeyPos::new(0x56, false)), Some(KeyValue::Char('ґ')));
}

#[test]
fn dead_key_positions_are_marked_dead() {
    let de = common::load("de");
    // ^ sits alone on the key left of 1; ´ and ` share the key right of ß, both dead.
    assert_eq!(de.value_at(KeyPos::new(0x29, false)), Some(KeyValue::Dead));
    assert_eq!(de.value_at(KeyPos::new(0x0d, false)), Some(KeyValue::Dead));
    assert_eq!(de.value_at(KeyPos::new(0x0d, true)), Some(KeyValue::Dead));
    assert!(de.is_dead(KeyPos::new(0x29, false)));
}

#[test]
fn a_dead_position_shifted_can_still_be_an_ordinary_character() {
    // Shift on the ^ key gives °, which is not dead. Dead-ness is a property of a
    // position, not of a physical key.
    let de = common::load("de");
    assert_eq!(de.value_at(KeyPos::new(0x29, true)), Some(KeyValue::Char('°')));
    assert!(!de.is_dead(KeyPos::new(0x29, true)));
}

#[test]
fn layouts_without_dead_keys_have_none() {
    for name in ["uk", "en_us"] {
        let table = common::load(name);
        assert!(
            !table.positions().any(|(_, v)| v == KeyValue::Dead),
            "{name} unexpectedly contains a dead position"
        );
    }
}

#[test]
fn characters_map_back_to_the_position_that_produces_them() {
    let de = common::load("de");
    assert_eq!(de.position_of('z'), Some(KeyPos::new(0x15, false)));
    assert_eq!(de.position_of('Z'), Some(KeyPos::new(0x15, true)));
    assert_eq!(de.position_of('ß'), Some(KeyPos::new(0x0c, false)));
}

#[test]
fn dead_positions_are_absent_from_the_reverse_map() {
    // A dead key has no character, so nothing can map back to it. `^` typed on German is
    // not a character in the table.
    let de = common::load("de");
    assert_eq!(de.position_of('^'), None);
    assert_eq!(de.position_of('´'), None);
}

#[test]
fn a_character_absent_from_a_layout_has_no_position() {
    let en = common::load("en_us");
    assert_eq!(en.position_of('ä'), None);
    assert_eq!(en.position_of('є'), None);
}

#[test]
fn unprobed_positions_are_absent_rather_than_empty() {
    // Function keys and the keypad were never probed; asking about them is a miss, not a
    // position that happens to produce nothing.
    let en = common::load("en_us");
    assert_eq!(en.value_at(KeyPos::new(0x3b, false)), None); // F1
    assert_eq!(en.value_at(KeyPos::new(0x00, false)), None);
}
