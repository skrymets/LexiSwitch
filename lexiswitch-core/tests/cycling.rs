//! Tasks 3.8 / 3.9 / 3.10 — multi-tap cycling.
//!
//! One rule covers target selection, disambiguation and undo
//! ([ADR-0009](../../docs/adr/0009-multi-tap-layout-cycling.md)): the nth press within the
//! window renders the retained source under the nth candidate, and after the last candidate
//! the cycle returns to the original. With two layouts installed that return *is* the
//! double-tap undo from the brief — not implemented separately.

mod common;

use lexiswitch_core::cycle::{render, MultiTap};
use lexiswitch_core::layout::LayoutTable;

#[test]
fn press_one_renders_the_first_candidate() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let candidates: Vec<&LayoutTable> = vec![&en];
    assert_eq!(render("ауфегкуі", &uk, &candidates, 1), "features");
}

#[test]
fn two_layouts_wrap_on_the_second_press() {
    // The undo case, arriving as a consequence of the general rule.
    let uk = common::load("uk");
    let en = common::load("en_us");
    let candidates: Vec<&LayoutTable> = vec![&en];

    assert_eq!(render("ауфегкуі", &uk, &candidates, 1), "features");
    assert_eq!(render("ауфегкуі", &uk, &candidates, 2), "ауфегкуі");
    // And it keeps cycling rather than sticking.
    assert_eq!(render("ауфегкуі", &uk, &candidates, 3), "features");
    assert_eq!(render("ауфегкуі", &uk, &candidates, 4), "ауфегкуі");
}

#[test]
fn three_layouts_wrap_on_the_third_press() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let de = common::load("de");
    let candidates: Vec<&LayoutTable> = vec![&en, &de];

    // `ня` sits on the two keys English and German disagree about. Most words render
    // identically under both — the layouts differ only at y/z and the umlaut keys — so a
    // word like `ауфегкуі` would make the three stops indistinguishable and prove nothing.
    assert_eq!(render("ня", &uk, &candidates, 1), "yz");
    assert_eq!(render("ня", &uk, &candidates, 2), "zy");
    assert_eq!(render("ня", &uk, &candidates, 3), "ня");
    assert_eq!(render("ня", &uk, &candidates, 4), "yz");
}

#[test]
fn every_press_renders_from_the_retained_original() {
    // Not from the previous result. Repeatedly converting already-converted text would
    // drift; re-rendering makes each press idempotent for its count.
    let en = common::load("en_us");
    let de = common::load("de");
    let candidates: Vec<&LayoutTable> = vec![&de];

    // `yz` -> `zy` under German. Rendering from the previous result would give `yz` back
    // on press 2 by accident; rendering from source gives it back by design, and the
    // distinction shows up with three candidates.
    assert_eq!(render("yz", &en, &candidates, 1), "zy");
    assert_eq!(render("yz", &en, &candidates, 2), "yz");
    assert_eq!(render("yz", &en, &candidates, 3), "zy");
}

#[test]
fn rendering_is_a_pure_function_of_press_count() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let candidates: Vec<&LayoutTable> = vec![&en];
    // Same inputs, same answer, in any order and any number of times.
    for _ in 0..3 {
        assert_eq!(render("ауфегкуі", &uk, &candidates, 2), "ауфегкуі");
        assert_eq!(render("ауфегкуі", &uk, &candidates, 1), "features");
    }
}

#[test]
fn press_zero_is_the_original() {
    let uk = common::load("uk");
    let en = common::load("en_us");
    let candidates: Vec<&LayoutTable> = vec![&en];
    assert_eq!(render("ауфегкуі", &uk, &candidates, 0), "ауфегкуі");
}

#[test]
fn no_candidates_means_nothing_to_cycle_through() {
    let uk = common::load("uk");
    let candidates: Vec<&LayoutTable> = vec![];
    for press in 0..4 {
        assert_eq!(render("ауфегкуі", &uk, &candidates, press), "ауфегкуі");
    }
}

// --- 3.9: the multi-tap window ---------------------------------------------------------

#[test]
fn a_first_press_starts_a_run() {
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(1_000), 1);
}

#[test]
fn a_press_within_the_window_continues_the_run() {
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(1_000), 1);
    assert_eq!(taps.press(1_150), 2);
    assert_eq!(taps.press(1_300), 3);
}

#[test]
fn a_press_beyond_the_window_starts_a_new_correction() {
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(1_000), 1);
    assert_eq!(taps.press(1_150), 2);
    // Deliberate pause: this is a new word, not the third candidate for the old one.
    assert_eq!(taps.press(4_000), 1);
}

#[test]
fn the_window_is_measured_from_the_previous_press_not_the_first() {
    // A slow but continuous run stays one run, which is what a user drumming the key does.
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(0), 1);
    assert_eq!(taps.press(400), 2);
    assert_eq!(taps.press(800), 3);
    assert_eq!(taps.press(1_200), 4);
}

#[test]
fn the_boundary_is_inclusive() {
    // Exactly at the window still counts as a continuation; the observed granularity is
    // 15.625 ms, so nothing lands on the boundary by accident.
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(0), 1);
    assert_eq!(taps.press(500), 2);

    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(0), 1);
    assert_eq!(taps.press(501), 1);
}

#[test]
fn observed_spike_timings_classify_as_intended() {
    // Straight from docs/spikes/2026-08-05-windows-input-primitives.md: deliberate
    // multi-taps landed at 141-157 ms, deliberate separations at 2531 ms and beyond.
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(676_585_453), 1);
    assert_eq!(taps.press(676_588_156), 1); // +2703 ms — new correction
    assert_eq!(taps.press(676_588_312), 2); // +156 ms  — continuation
    assert_eq!(taps.press(676_590_843), 1); // +2531 ms — new correction
    assert_eq!(taps.press(676_590_984), 2); // +141 ms
    assert_eq!(taps.press(676_591_125), 3); // +141 ms
}

#[test]
fn the_run_can_be_reset_explicitly() {
    // A conversion committed, focus moved, or the buffer wiped: the next press starts over.
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(1_000), 1);
    assert_eq!(taps.press(1_100), 2);
    taps.reset();
    assert_eq!(taps.press(1_200), 1);
}

#[test]
fn tick_rollover_does_not_extend_a_run() {
    let mut taps = MultiTap::new(500);
    assert_eq!(taps.press(u32::MAX - 100), 1);
    assert_eq!(taps.press(u32::MAX.wrapping_add(100)), 2); // 200 ms later across the wrap
    assert_eq!(taps.press(u32::MAX.wrapping_add(5_000)), 1);
}
