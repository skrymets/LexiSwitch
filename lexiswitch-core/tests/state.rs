//! What the agent is currently able to do, and what it should say about it.
//!
//! The program used to have two ways of ending before it started: a shortcut another
//! application already held, and fewer than two installed layouts. Both were `?` at startup, so
//! the user got an exit code and no tray icon — which is indistinguishable from the program
//! being broken, and is exactly what happened during testing when a stale instance held the
//! hotkey.
//!
//! Both are now states the program carries while running. This is the logic that decides which
//! one it is in and what that means; it knows nothing about Windows.

use lexiswitch_core::state::{AgentState, Shortcut};

/// The ordinary case: two layouts, a registered shortcut, switched on.
#[test]
fn a_healthy_agent_converts() {
    let s = AgentState::resolve(3, Shortcut::Registered("Shift+Pause".into()), true, None);
    assert_eq!(
        s,
        AgentState::Enabled {
            binding: "Shift+Pause".into()
        }
    );
    assert!(s.converts());
}

/// Turning it off is the user's choice, and it is not an error.
#[test]
fn disabled_does_not_convert_and_is_not_a_problem() {
    let s = AgentState::resolve(3, Shortcut::Registered("Shift+Pause".into()), false, None);
    assert_eq!(s, AgentState::Disabled);
    assert!(!s.converts());
    assert!(!s.needs_attention());
}

/// One layout means there is nothing to convert *into*. The program stays reachable and says so
/// rather than exiting, which is what the spec requires and what the old `?` did not do.
#[test]
fn one_layout_disables_conversion_but_not_the_program() {
    let s = AgentState::resolve(1, Shortcut::Registered("Shift+Pause".into()), true, None);
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
    assert!(!s.converts());
    assert!(s.needs_attention());
    assert!(s.explanation().contains("two"));
}

/// A held shortcut stops the *shortcut*, not the program. Conversion is still possible from the
/// tray, so this state converts — otherwise a conflict would leave a running program that can
/// do nothing at all.
#[test]
fn a_shortcut_conflict_still_allows_conversion() {
    let s = AgentState::resolve(3, Shortcut::Conflict("Shift+Pause".into()), true, None);
    assert_eq!(
        s,
        AgentState::ShortcutConflict {
            binding: "Shift+Pause".into()
        }
    );
    assert!(s.converts());
    assert!(s.needs_attention());
    assert!(!s.shortcut_works());
}

/// The combination is named, because "the shortcut is taken" is not actionable without knowing
/// which one.
#[test]
fn a_conflict_names_the_combination() {
    let s = AgentState::resolve(3, Shortcut::Conflict("Ctrl+Alt+K".into()), true, None);
    assert!(s.explanation().contains("Ctrl+Alt+K"));
}

/// A runtime failure is distinct from every other state and carries its own detail.
#[test]
fn a_runtime_error_is_its_own_state() {
    let s = AgentState::resolve(
        3,
        Shortcut::Registered("Shift+Pause".into()),
        true,
        Some("layout tables could not be built".into()),
    );
    assert!(matches!(s, AgentState::RuntimeError { .. }));
    assert!(!s.converts());
    assert!(s.needs_attention());
    assert!(s.explanation().contains("layout tables"));
}

/// Precedence matters because several conditions can hold at once, and the user should be told
/// about the one that blocks the most. Too few layouts outranks a shortcut conflict: fixing the
/// shortcut first would leave a working shortcut that still cannot convert anything.
#[test]
fn the_most_blocking_condition_wins() {
    let s = AgentState::resolve(1, Shortcut::Conflict("Shift+Pause".into()), true, None);
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
}

/// A runtime error outranks everything: nothing else can be trusted while it holds.
#[test]
fn a_runtime_error_outranks_the_rest() {
    let s = AgentState::resolve(
        1,
        Shortcut::Conflict("Shift+Pause".into()),
        false,
        Some("boom".into()),
    );
    assert!(matches!(s, AgentState::RuntimeError { .. }));
}

/// Being switched off does not hide a condition the user has to act on, or turning it back on
/// would be a surprise.
#[test]
fn disabled_does_not_mask_a_blocking_condition() {
    let s = AgentState::resolve(1, Shortcut::Registered("Shift+Pause".into()), false, None);
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
}

/// Every state says something different, and none of them is empty. The tray shows these, and a
/// state indistinguishable from another is a state the user cannot act on.
#[test]
fn every_state_is_distinguishable() {
    let states = [
        AgentState::resolve(3, Shortcut::Registered("Shift+Pause".into()), true, None),
        AgentState::resolve(3, Shortcut::Registered("Shift+Pause".into()), false, None),
        AgentState::resolve(1, Shortcut::Registered("Shift+Pause".into()), true, None),
        AgentState::resolve(3, Shortcut::Conflict("Shift+Pause".into()), true, None),
        AgentState::resolve(
            3,
            Shortcut::Registered("Shift+Pause".into()),
            true,
            Some("boom".into()),
        ),
    ];

    let mut summaries: Vec<String> = states.iter().map(|s| s.summary()).collect();
    summaries.sort();
    summaries.dedup();
    assert_eq!(summaries.len(), 5, "two states share a summary");

    for s in &states {
        assert!(!s.summary().is_empty());
        assert!(!s.explanation().is_empty());
    }
}

/// The tooltip names the shortcut when there is a working one, because the whole interface is
/// otherwise invisible: a tray icon with no hint of how to invoke it teaches nothing.
#[test]
fn the_summary_names_a_working_shortcut() {
    let s = AgentState::resolve(2, Shortcut::Registered("Ctrl+Alt+K".into()), true, None);
    assert!(s.summary().contains("Ctrl+Alt+K"), "{}", s.summary());
}
