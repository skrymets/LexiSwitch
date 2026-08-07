//! Compacting a candidate for a chooser row.
//!
//! A row has a fixed width and a selection does not. What gets shown has to be the part that
//! answers the question the chooser is asking — *is this the conversion I meant?* — which is
//! the part that differs from the original, not the part that happens to come first.
//!
//! Pure logic, no Windows: the chooser draws the string this produces and decides nothing.

use lexiswitch_core::preview::compact;

/// Text that fits is shown whole, with nothing hidden and nothing to signal.
#[test]
fn short_text_is_untouched() {
    let p = compact("my super text", "ьн ыгзук ершт", 40);
    assert_eq!(p.text, "my super text");
    assert!(!p.elided_start);
    assert!(!p.elided_end);
}

/// The window is at most `width` characters. Exceeding it would overflow the row it was
/// measured for, which is the one thing this function exists to prevent.
#[test]
fn the_window_never_exceeds_the_width() {
    let original = "a".repeat(300);
    let converted = format!("{}X{}", "a".repeat(150), "a".repeat(149));
    let p = compact(&converted, &original, 24);
    assert!(
        p.text.chars().count() <= 24,
        "{} chars exceeds the 24 asked for",
        p.text.chars().count()
    );
}

/// The point of the whole exercise: a difference 200 characters in must be on screen. Showing
/// the first 24 characters of two candidates that differ only at the end shows the user two
/// identical rows and asks them to choose.
#[test]
fn a_late_change_is_inside_the_window() {
    let original = format!("{}щдв", "same text ".repeat(20));
    let converted = format!("{}old", "same text ".repeat(20));
    let p = compact(&converted, &original, 24);
    assert!(
        p.text.contains("old"),
        "the changed region is missing from {:?}",
        p.text
    );
    assert!(p.elided_start, "content before the window was not signalled");
}

/// An early change keeps the start visible rather than centring for its own sake.
#[test]
fn an_early_change_keeps_the_start() {
    let original = format!("ьн{}", " padding".repeat(20));
    let converted = format!("my{}", " padding".repeat(20));
    let p = compact(&converted, &original, 24);
    assert!(p.text.starts_with("my"), "{:?}", p.text);
    assert!(!p.elided_start);
    assert!(p.elided_end);
}

/// Omission is reported, never silent. A preview that quietly drops content is indistinguishable
/// from a conversion that produced less text than it did.
#[test]
fn omission_is_always_signalled() {
    let original = "x".repeat(100);
    let converted = "y".repeat(100);
    let p = compact(&converted, &original, 20);
    assert!(p.elided_start || p.elided_end);
}

/// Identical strings have no changed region to favour, and must not panic looking for one.
#[test]
fn no_difference_falls_back_to_the_start() {
    let text = "unchanged ".repeat(10);
    let p = compact(&text, &text, 16);
    assert!(text.starts_with(&p.text), "{:?}", p.text);
    assert!(!p.elided_start);
    assert!(p.elided_end);
}

/// Character counts, not byte counts. Cyrillic is two bytes per character in UTF-8 and the
/// row is measured in characters; slicing on bytes would both overflow the row and panic on a
/// boundary.
#[test]
fn width_counts_characters_not_bytes() {
    let original = "англійський текст ".repeat(6);
    let converted = "фтпдsqcmrbq ntrcn ".repeat(6);
    let p = compact(&converted, &original, 20);
    assert!(p.text.chars().count() <= 20);
    assert!(p.text.chars().count() >= 10, "{:?} is uselessly short", p.text);
}

/// A width no sane caller would pass still has to return something slice-safe.
#[test]
fn degenerate_widths_do_not_panic() {
    for width in [0usize, 1, 2, 3] {
        let p = compact("converted text", "original text", width);
        assert!(p.text.chars().count() <= width.max(1));
    }
}

/// The changed region is measured against the original, so a caller that passes a different
/// comparison string gets a window around *that* difference. This is what lets the expanded
/// view and the row use the same function with different widths.
#[test]
fn the_window_is_stable_across_widths() {
    let original = format!("{}щдв", "same text ".repeat(20));
    let converted = format!("{}old", "same text ".repeat(20));
    let narrow = compact(&converted, &original, 24);
    let wide = compact(&converted, &original, 60);
    assert!(narrow.text.contains("old"));
    assert!(wide.text.contains("old"));
    assert!(wide.text.chars().count() > narrow.text.chars().count());
}

// --- insertions and deletions --------------------------------------------------------
//
// Layout conversion maps character for character, so position *i* of a candidate always
// corresponds to position *i* of the original. Case conversion does not: `snake_case` inserts
// separators, stripping removes characters, and German `ß` uppercases to two characters. A
// positional comparison against text that has shifted reports the first difference at the
// point of the shift and the last difference at the end of the string, which is a window over
// everything after the change rather than over the change.

/// A separator inserted early must not make the whole remainder look changed.
#[test]
fn an_insertion_does_not_widen_the_changed_region() {
    let original = format!("word {}", "padding ".repeat(30));
    let converted = format!("word_{}", "padding ".repeat(30));
    let p = compact(&converted, &original, 24);
    assert!(
        p.text.starts_with("word_"),
        "the window should sit on the inserted separator, not after it: {:?}",
        p.text
    );
    assert!(!p.elided_start);
}

/// A late insertion is still found, even though every character after it has moved.
#[test]
fn a_late_insertion_is_inside_the_window() {
    let original = format!("{}end here", "same text ".repeat(20));
    let converted = format!("{}end_here", "same text ".repeat(20));
    let p = compact(&converted, &original, 24);
    assert!(
        p.text.contains("end_here"),
        "the changed region is missing from {:?}",
        p.text
    );
    assert!(p.elided_start);
}

/// The German case that grows the string: `ß` uppercases to `SS`, so everything after it
/// shifts by one and a positional comparison sees the entire tail as different.
#[test]
fn a_growing_transform_locates_the_growth() {
    let original = format!("{}straße{}", "x ".repeat(20), " tail".repeat(6));
    let converted = format!("{}STRASSE{}", "x ".repeat(20), " tail".repeat(6));
    let p = compact(&converted, &original, 24);
    assert!(
        p.text.contains("STRASSE"),
        "the growth is missing from {:?}",
        p.text
    );
}

/// A deletion is the mirror of an insertion and must behave the same way.
#[test]
fn a_deletion_does_not_widen_the_changed_region() {
    let original = format!("{}a-b{}", "pad ".repeat(20), " tail".repeat(8));
    let converted = format!("{}ab{}", "pad ".repeat(20), " tail".repeat(8));
    let p = compact(&converted, &original, 20);
    assert!(p.text.contains("ab"), "{:?}", p.text);
    assert!(
        p.elided_end,
        "content after the window was not signalled: {p:?}"
    );
}
