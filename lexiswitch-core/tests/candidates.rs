//! Candidate generation: which conversions are worth offering for a selection.
//!
//! The direction of a conversion used to come from whichever layout happened to be active in
//! the foreground window, which is not evidence of what the text was typed under. These tests
//! pin the replacement: every plausible conversion is derived from the characters themselves.

mod common;

use lexiswitch_core::candidates::{self, Candidate};
use lexiswitch_core::layout::LayoutTable;

fn tables() -> Vec<LayoutTable> {
    vec![common::load("en_us"), common::load("de"), common::load("uk")]
}

/// Typed with the Ukrainian layout active while intending English.
const CYRILLIC: &str = "Ерші ші ьн ігзук еуче";
const ENGLISH: &str = "This is my super text";

fn labels(found: &[Candidate]) -> Vec<String> {
    found
        .iter()
        .map(|c| format!("{}->{}", c.from, c.to))
        .collect()
}

#[test]
fn no_candidate_converts_a_layout_to_itself() {
    let found = candidates::generate(CYRILLIC, &tables(), None);
    assert!(
        found.iter().all(|c| c.from != c.to),
        "self-pairs must never be offered, got {:?}",
        labels(&found)
    );
}

#[test]
fn a_source_that_cannot_produce_the_characters_yields_nothing() {
    // en-US cannot produce Cyrillic, so every character passes through and the result is the
    // original — which is not a conversion and must not be offered.
    let found = candidates::generate(CYRILLIC, &tables(), None);
    assert!(
        found.iter().all(|c| c.from == "uk"),
        "only Ukrainian can have produced this text, got {:?}",
        labels(&found)
    );
    assert!(!found.is_empty(), "uk is a plausible source and must be offered");
}

#[test]
fn no_candidate_leaves_the_text_unchanged() {
    let found = candidates::generate(CYRILLIC, &tables(), None);
    assert!(
        found.iter().all(|c| c.text != CYRILLIC),
        "a candidate identical to the original is not a conversion"
    );
}

#[test]
fn text_that_converts_to_nothing_anywhere_yields_no_candidates() {
    // Digits sit on the same positions in every installed layout.
    let found = candidates::generate("12345", &tables(), None);
    assert!(
        found.is_empty(),
        "expected no distinct conversion, got {:?}",
        labels(&found)
    );
}

#[test]
fn candidates_with_the_same_text_but_different_targets_are_both_kept() {
    // The target decides what an apply-and-switch activates, so collapsing on visible text
    // alone would silently discard a choice the user is being asked to make.
    let found = candidates::generate(CYRILLIC, &tables(), None);
    let mut same_text: Vec<&Candidate> = Vec::new();
    for c in &found {
        if found.iter().filter(|o| o.text == c.text).count() > 1 {
            same_text.push(c);
        }
    }
    for c in &same_text {
        assert!(
            same_text.iter().filter(|o| o.to == c.to).count() == 1,
            "two candidates share a result AND a target, which should have collapsed"
        );
    }
}

#[test]
fn candidates_with_the_same_text_and_target_collapse() {
    let found = candidates::generate(ENGLISH, &tables(), None);
    let mut seen: Vec<(&str, &str)> = found.iter().map(|c| (c.text.as_str(), c.to.as_str())).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(before, seen.len(), "result-and-target pairs must be unique");
}

#[test]
fn a_uniquely_identifying_alphabet_ranks_its_source_first() {
    let found = candidates::generate(CYRILLIC, &tables(), None);
    assert_eq!(
        found.first().map(|c| c.from.as_str()),
        Some("uk"),
        "Cyrillic is producible only by uk, so uk-sourced candidates lead: {:?}",
        labels(&found)
    );
}

#[test]
fn an_ambiguous_alphabet_still_produces_a_stable_order() {
    // Latin cannot separate en-US from de-DE, and no dictionary is permitted to help. The
    // requirement is determinism, not correctness — the user's eye is the tiebreaker.
    let a = labels(&candidates::generate(ENGLISH, &tables(), None));
    let b = labels(&candidates::generate(ENGLISH, &tables(), None));
    assert_eq!(a, b, "ordering must not vary between runs");
    assert!(a.len() > 1, "expected several candidates for Latin text, got {a:?}");
}

#[test]
fn unchanged_characters_are_counted_per_candidate() {
    // Spaces are unmappable in every direction, so each one is reported unchanged.
    let found = candidates::generate(CYRILLIC, &tables(), None);
    let spaces = CYRILLIC.chars().filter(|c| *c == ' ').count();
    for c in &found {
        assert!(
            c.unchanged >= spaces,
            "{}->{} reported {} unchanged, expected at least the {spaces} spaces",
            c.from,
            c.to,
            c.unchanged
        );
        assert!(c.unchanged < CYRILLIC.chars().count(), "a fully unchanged candidate should have been dropped");
    }
}

#[test]
fn every_candidate_is_generated_from_the_original_text() {
    // Not from another candidate's result: converting a result again would compound rather
    // than offer an alternative.
    let found = candidates::generate(CYRILLIC, &tables(), None);
    let tables = tables();
    for c in &found {
        let from = tables.iter().find(|t| t.name() == c.from).unwrap();
        let to = tables.iter().find(|t| t.name() == c.to).unwrap();
        assert_eq!(
            c.text,
            lexiswitch_core::convert::convert_lossy(CYRILLIC, from, to),
            "{}->{} does not match converting the original",
            c.from,
            c.to
        );
    }
}

#[test]
fn the_active_layout_breaks_a_tie_on_the_target() {
    // Cyrillic names uk as the source outright and then says nothing about which Latin layout
    // the user wants back. Without a tiebreak the winner is whichever sorts first, which is
    // how the commonest case ends up preselecting the wrong candidate.
    let with_en = candidates::generate(CYRILLIC, &tables(), Some("en_us"));
    assert_eq!(with_en.first().map(|c| c.to.as_str()), Some("en_us"));

    let with_de = candidates::generate(CYRILLIC, &tables(), Some("de"));
    assert_eq!(with_de.first().map(|c| c.to.as_str()), Some("de"));
}

#[test]
fn the_active_layout_never_chooses_a_source() {
    // The whole point of the replacement: the active layout is not evidence about text that
    // may have been typed at any time. Claiming en-US is active must not make it a source for
    // characters it cannot produce.
    let found = candidates::generate(CYRILLIC, &tables(), Some("en_us"));
    assert!(
        found.iter().all(|c| c.from == "uk"),
        "the hint leaked into source selection: {:?}",
        labels(&found)
    );
}

#[test]
fn the_active_layout_cannot_conjure_a_candidate() {
    // A hint must never turn "no distinct conversion" into one.
    assert!(candidates::generate("12345", &tables(), Some("en_us")).is_empty());
}
