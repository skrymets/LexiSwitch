//! Task 2.9 — round trip A→B→A is identity for text mappable in both layouts.
//!
//! Exhaustive rather than randomised. The input space is tiny and fully known — 48 keys in
//! two shift states, three layouts — so every character in every ordered pair can be checked
//! directly. A generator would sample the same space less completely and pull in a
//! dependency the binary budget does not need.
//!
//! The property has one genuine exception, pinned down by its own test below: where the
//! target layout produces one character from two different keys, the inverse cannot be
//! injective and the original key is unrecoverable.

mod common;

use lexiswitch_core::convert::convert_lossy;
use lexiswitch_core::layout::{KeyPos, KeyValue};

const LAYOUTS: [&str; 3] = ["uk", "en_us", "de"];

#[test]
fn roundtrip_is_identity_for_every_unambiguously_mappable_character() {
    for a_name in LAYOUTS {
        for b_name in LAYOUTS {
            if a_name == b_name {
                continue;
            }
            let a = common::load(a_name);
            let b = common::load(b_name);
            let (mut checked, mut ambiguous) = (0, 0);

            for (pos, value) in a.positions() {
                let KeyValue::Char(c) = value else { continue };
                // Mappable in both directions. Dead and unprobed positions pass through
                // and are covered by the pass-through tests instead.
                let Some(target) = b.char_at(pos) else { continue };

                // If the target produces this character from a different key than the one
                // we arrived on, the target has duplicates and the trip home is ambiguous.
                if b.position_of(target) != Some(pos) {
                    ambiguous += 1;
                    continue;
                }

                let text = c.to_string();
                let there = convert_lossy(&text, &a, &b);
                let back = convert_lossy(&there, &b, &a);
                assert_eq!(
                    back, text,
                    "{a_name} -> {b_name} -> {a_name} lost {c:?} at {pos:?} (via {there:?})"
                );
                checked += 1;
            }

            assert!(
                checked > 40,
                "{a_name} -> {b_name} checked only {checked} characters"
            );
            // Guards the exception from quietly widening: only en_us duplicates anything,
            // and only on the ISO extra key in two shift states.
            let expected_ambiguous = if b_name == "en_us" { 2 } else { 0 };
            assert_eq!(
                ambiguous, expected_ambiguous,
                "{a_name} -> {b_name} had {ambiguous} ambiguous characters, expected {expected_ambiguous}"
            );
        }
    }
}

#[test]
fn a_character_the_target_produces_from_two_keys_cannot_round_trip() {
    let uk = common::load("uk");
    let en = common::load("en_us");

    // US English reaches `\` from both the backslash key (0x2b) and the ISO extra key
    // (0x56), which duplicates it on ISO keyboards. Ukrainian puts ґ on that extra key.
    assert_eq!(en.char_at(KeyPos::new(0x2b, false)), Some('\\'));
    assert_eq!(en.char_at(KeyPos::new(0x56, false)), Some('\\'));
    assert_eq!(uk.char_at(KeyPos::new(0x56, false)), Some('ґ'));

    // So ґ converts to `\` correctly, but `\` cannot say which key it came from, and the
    // trip home lands on the backslash key instead.
    assert_eq!(convert_lossy("ґ", &uk, &en), "\\");
    assert_eq!(convert_lossy("\\", &en, &uk), "\\");

    // This cannot bite in practice. Multi-tap cycling re-renders every press from the
    // retained source text rather than from the previous result (ADR-0009), so a
    // conversion is never fed its own output. That decision was made for a different
    // reason; this is a second thing it happens to prevent.
}

#[test]
fn roundtrip_holds_for_whole_strings_in_their_own_source_layout() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let de = common::load("de");

    // Each string is paired with a layout that can actually produce it — text the source
    // cannot type passes straight through, which makes the forward leg a no-op and the
    // property vacuous.
    let cases: [(&str, &_, &_); 5] = [
        ("ауфегкуі", &uk, &en),
        ("привіт світ", &uk, &en),
        ("features", &en, &de),
        ("the quick brown fox", &en, &de),
        ("Mixed CASE 123", &en, &de),
    ];

    for (text, a, b) in cases {
        let there = convert_lossy(text, a, b);
        let back = convert_lossy(&there, b, a);
        assert_eq!(back, text, "round trip failed for {text:?} via {there:?}");
    }
}
