//! Which case styles the user actually reaches for, so the three they use most sit on top.
//!
//! Text only — this knows nothing about files. The binary reads and writes; this parses and
//! renders, which is the same split `Config` has and for the same reason: the interesting rules
//! are testable without a filesystem.
//!
//! What may be in it is closed, not open-ended
//! ([ADR-0015](../../docs/adr/0015-usage-memory-outside-configuration.md)): style identifiers
//! this program defines, and integers. Nothing derived from what the user typed.

use lexiswitch_core::casing::Style;
use lexiswitch_core::usage::Usage;

/// A fresh memory promotes nothing and claims nothing.
#[test]
fn an_empty_memory_promotes_the_declared_order() {
    let usage = Usage::default();
    assert!(usage.is_empty());
    assert_eq!(usage.top(3), Vec::<Style>::new());
}

/// The most-applied style comes first.
#[test]
fn the_most_used_style_leads() {
    let mut usage = Usage::default();
    usage.record(Style::Snake);
    usage.record(Style::Upper);
    usage.record(Style::Upper);
    assert_eq!(usage.top(3), vec![Style::Upper, Style::Snake]);
}

/// Only styles that have been used are promoted. Padding the list to three with styles nobody
/// has chosen would put arbitrary rows in the most valuable position on screen.
#[test]
fn only_used_styles_are_promoted() {
    let mut usage = Usage::default();
    usage.record(Style::Title);
    assert_eq!(usage.top(3), vec![Style::Title]);
}

/// The count asked for is a maximum.
#[test]
fn the_promoted_list_is_bounded() {
    let mut usage = Usage::default();
    for style in Style::ALL {
        usage.record(style);
    }
    assert_eq!(usage.top(3).len(), 3);
    assert_eq!(usage.top(0).len(), 0);
}

/// Equal counts break by the declared order, so the promoted list never depends on the order
/// things happened to be recorded in and never varies between runs.
#[test]
fn ties_break_by_the_declared_order() {
    let mut a = Usage::default();
    a.record(Style::Title);
    a.record(Style::Lower);

    let mut b = Usage::default();
    b.record(Style::Lower);
    b.record(Style::Title);

    assert_eq!(a.top(2), b.top(2));
    // `Lower` is declared before `Title`, so it wins the tie whichever order they arrived in.
    assert_eq!(a.top(2), vec![Style::Lower, Style::Title]);
}

/// Discarding the counts is what the window's in-place reset does, and it must leave nothing
/// behind — a memory that has learned the wrong thing has to be genuinely forgettable.
#[test]
fn resetting_forgets_everything() {
    let mut usage = Usage::default();
    usage.record(Style::Upper);
    usage.record(Style::Upper);
    usage.reset();
    assert!(usage.is_empty());
    assert_eq!(usage.top(3), Vec::<Style>::new());
}

/// Counts are bounded rather than wrapping. Nobody will reach this at human speed; saturating
/// costs nothing and means the question never has to be thought about again.
#[test]
fn counts_saturate_rather_than_wrap() {
    let mut usage = Usage::default();
    for _ in 0..5 {
        usage.record(Style::Upper);
    }
    let text = usage.render().replace("upper = 5", &format!("upper = {}", u32::MAX));
    let mut reloaded = Usage::parse(&text);
    reloaded.record(Style::Upper);
    reloaded.record(Style::Upper);
    assert_eq!(reloaded.top(1), vec![Style::Upper]);
}

// --- the text form -------------------------------------------------------------------

/// What is written can be read back.
#[test]
fn rendering_round_trips_through_parsing() {
    let mut usage = Usage::default();
    usage.record(Style::Snake);
    usage.record(Style::Snake);
    usage.record(Style::Camel);

    let reloaded = Usage::parse(&usage.render());
    assert_eq!(reloaded.top(3), usage.top(3));
}

/// The file holds identifiers and integers, and the claim is checkable by looking at it rather
/// than by trusting a description of it.
#[test]
fn the_rendered_form_holds_only_identifiers_and_integers() {
    let mut usage = Usage::default();
    usage.record(Style::ScreamingSnake);
    let text = usage.render();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap_or_else(|| panic!("{line:?}"));
        assert!(
            Style::from_id(key.trim()).is_some(),
            "{key:?} is not a style this program defines"
        );
        assert!(
            value.trim().parse::<u32>().is_ok(),
            "{value:?} is not a count"
        );
    }
}

/// Rubbish is "no memory yet", not an error. There is nothing in this file worth refusing to
/// start over.
#[test]
fn unusable_text_parses_as_empty() {
    for text in ["", "   ", "not toml at all {{{", "[case_styles]\nupper = \"lots\"\n"] {
        assert!(Usage::parse(text).is_empty(), "{text:?} was not treated as empty");
    }
}

/// A style this version does not have is ignored for ordering — but not thrown away, so
/// running an older build once does not destroy a newer one's counts.
#[test]
fn an_unknown_style_is_ignored_but_kept() {
    let text = "[case_styles]\nupper = 4\nmorse_code = 9\n";
    let usage = Usage::parse(text);
    assert_eq!(usage.top(3), vec![Style::Upper], "an unknown style was promoted");
    assert!(
        usage.render().contains("morse_code = 9"),
        "a later version's count was discarded: {}",
        usage.render()
    );
}

/// A count that is not a number is dropped rather than guessed at.
#[test]
fn a_non_numeric_count_is_dropped() {
    let usage = Usage::parse("[case_styles]\nupper = 4\nsnake = true\n");
    assert_eq!(usage.top(3), vec![Style::Upper]);
}

/// An empty memory still renders something loadable, so the first write is not a special case.
#[test]
fn an_empty_memory_renders_something_parseable() {
    let text = Usage::default().render();
    assert!(Usage::parse(&text).is_empty());
}
