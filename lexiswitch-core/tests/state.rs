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
//!
//! There are two shortcuts now, one per transaction
//! ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)), so a
//! conflict is no longer a single fact — it has to name which combination is unavailable and
//! say what still works.

use lexiswitch_core::state::{AgentState, Shortcut, Shortcuts};

fn all_registered() -> Shortcuts {
    Shortcuts {
        convert: Shortcut::Registered("Shift+Pause".into()),
        case: Shortcut::Registered("Win+Ctrl+Pause".into()),
        translit: Shortcut::Registered("Win+Alt+Pause".into()),
    }
}

/// The ordinary case: two layouts, both shortcuts registered, switched on.
#[test]
fn a_healthy_agent_converts() {
    let s = AgentState::resolve(3, all_registered(), true, None);
    assert_eq!(
        s,
        AgentState::Enabled {
            convert: "Shift+Pause".into(),
            case: "Win+Ctrl+Pause".into(),
            translit: "Win+Alt+Pause".into(),
        }
    );
    assert!(s.converts());
}

/// Turning it off is the user's choice, and it is not an error.
#[test]
fn disabled_does_not_convert_and_is_not_a_problem() {
    let s = AgentState::resolve(3, all_registered(), false, None);
    assert_eq!(s, AgentState::Disabled);
    assert!(!s.converts());
    assert!(!s.needs_attention());
}

/// One layout means there is nothing to convert *into*. The program stays reachable and says so
/// rather than exiting, which is what the spec requires and what the old `?` did not do.
#[test]
fn one_layout_disables_conversion_but_not_the_program() {
    let s = AgentState::resolve(1, all_registered(), true, None);
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
    assert!(!s.converts());
    assert!(s.needs_attention());
    assert!(s.explanation().contains("two"));
}

/// A held shortcut stops the *shortcut*, not the program. Both transactions are still reachable
/// from the tray, so this state converts — otherwise a conflict would leave a running program
/// that can do nothing at all.
#[test]
fn a_shortcut_conflict_still_allows_conversion() {
    let s = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Conflict("Shift+Pause".into()),
            case: Shortcut::Registered("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    assert!(matches!(s, AgentState::ShortcutConflict { .. }));
    assert!(s.converts());
    assert!(s.needs_attention());
    assert!(!s.shortcut_works());
}

/// The combination is named, because "the shortcut is taken" is not actionable without knowing
/// which one.
#[test]
fn a_conflict_names_the_combination() {
    let s = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Conflict("Ctrl+Alt+K".into()),
            case: Shortcut::Registered("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    assert!(s.explanation().contains("Ctrl+Alt+K"));
}

// --- two shortcuts -------------------------------------------------------------------

/// One unavailable combination must not be reported as though everything is broken. The other
/// shortcut is working, and saying so is the difference between a user who knows what to fix
/// and one who thinks the program is dead.
#[test]
fn one_conflict_says_what_still_works() {
    let s = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Registered("Shift+Pause".into()),
            case: Shortcut::Conflict("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    let explanation = s.explanation();
    assert!(
        explanation.contains("Win+Ctrl+Pause"),
        "the unavailable combination is not named: {explanation}"
    );
    assert!(
        explanation.contains("Shift+Pause"),
        "the working combination is not named: {explanation}"
    );
}

/// The state distinguishes which one failed. A conflict on the case shortcut and a conflict on
/// the conversion shortcut are different problems with different consequences.
#[test]
fn a_conflict_records_which_shortcut_is_unavailable() {
    let case_only = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Registered("Shift+Pause".into()),
            case: Shortcut::Conflict("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    let convert_only = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Conflict("Shift+Pause".into()),
            case: Shortcut::Registered("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    assert_ne!(case_only, convert_only);
    assert_ne!(case_only.summary(), convert_only.summary());
}

/// When every one is held there is nothing to reassure the user about, and the message must not
/// invent something that still works.
#[test]
fn all_conflicting_claims_no_working_shortcut() {
    let s = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Conflict("Shift+Pause".into()),
            case: Shortcut::Conflict("Win+Ctrl+Pause".into()),
            translit: Shortcut::Conflict("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    match &s {
        AgentState::ShortcutConflict {
            unavailable,
            working,
        } => {
            assert_eq!(unavailable.len(), 3);
            assert!(working.is_empty(), "{working:?}");
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    // Both transactions are still reachable from the tray, which is the whole reason the tray
    // commands exist.
    assert!(s.converts());
    assert!(s.explanation().to_lowercase().contains("tray"));
}

/// The tooltip names both shortcuts when both work: this is the only place either is visible
/// without opening the configuration file.
#[test]
fn the_summary_names_both_working_shortcuts() {
    let s = AgentState::resolve(2, all_registered(), true, None);
    let summary = s.summary();
    assert!(summary.contains("Shift+Pause"), "{summary}");
    assert!(summary.contains("Win+Ctrl+Pause"), "{summary}");
}

/// A tray tooltip is truncated by Windows past 128 characters, so a message that names two
/// combinations still has to fit in one.
#[test]
fn summaries_fit_a_tray_tooltip() {
    let states = [
        AgentState::resolve(3, all_registered(), true, None),
        AgentState::resolve(
            3,
            Shortcuts {
                convert: Shortcut::Conflict("Ctrl+Alt+Shift+Insert".into()),
                case: Shortcut::Conflict("Ctrl+Alt+Shift+PageDown".into()),
                translit: Shortcut::Registered("Win+Alt+Pause".into()),
            },
            true,
            None,
        ),
    ];
    for s in &states {
        let n = s.summary().chars().count();
        assert!(n <= 127, "{n} characters is too long for a tooltip: {}", s.summary());
    }
}

// --- precedence ----------------------------------------------------------------------

/// A runtime failure is distinct from every other state and carries its own detail.
#[test]
fn a_runtime_error_is_its_own_state() {
    let s = AgentState::resolve(
        3,
        all_registered(),
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
    let s = AgentState::resolve(
        1,
        Shortcuts {
            convert: Shortcut::Conflict("Shift+Pause".into()),
            case: Shortcut::Registered("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        true,
        None,
    );
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
}

/// A runtime error outranks everything: nothing else can be trusted while it holds.
#[test]
fn a_runtime_error_outranks_the_rest() {
    let s = AgentState::resolve(
        1,
        Shortcuts {
            convert: Shortcut::Conflict("Shift+Pause".into()),
            case: Shortcut::Conflict("Win+Ctrl+Pause".into()),
            translit: Shortcut::Registered("Win+Alt+Pause".into()),
        },
        false,
        Some("boom".into()),
    );
    assert!(matches!(s, AgentState::RuntimeError { .. }));
}

/// Being switched off does not hide a condition the user has to act on, or turning it back on
/// would be a surprise.
#[test]
fn disabled_does_not_mask_a_blocking_condition() {
    let s = AgentState::resolve(1, all_registered(), false, None);
    assert_eq!(s, AgentState::TooFewLayouts { installed: 1 });
}

/// Every state says something different, and none of them is empty. The tray shows these, and a
/// state indistinguishable from another is a state the user cannot act on.
#[test]
fn every_state_is_distinguishable() {
    let states = [
        AgentState::resolve(3, all_registered(), true, None),
        AgentState::resolve(3, all_registered(), false, None),
        AgentState::resolve(1, all_registered(), true, None),
        AgentState::resolve(
            3,
            Shortcuts {
                convert: Shortcut::Conflict("Shift+Pause".into()),
                case: Shortcut::Registered("Win+Ctrl+Pause".into()),
                translit: Shortcut::Registered("Win+Alt+Pause".into()),
            },
            true,
            None,
        ),
        AgentState::resolve(3, all_registered(), true, Some("boom".into())),
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

// --- suggestions ---------------------------------------------------------------------

/// Suggestions never include the combination the user already has, which is the one thing
/// known not to work.
#[test]
fn suggestions_exclude_the_current_shortcut() {
    let s = lexiswitch_core::state::suggested_shortcuts(&["Ctrl+Alt+K"]);
    assert!(!s.iter().any(|x| x == "Ctrl+Alt+K"), "{s:?}");
    assert!(!s.is_empty());
}

/// Nor the program's *other* shortcut. Suggesting it would offer a combination that is
/// guaranteed to be refused — one combination cannot open two windows — and the refusal would
/// name LexiSwitch itself, which reads as a bug rather than as advice.
#[test]
fn suggestions_exclude_every_shortcut_the_program_holds() {
    let s = lexiswitch_core::state::suggested_shortcuts(&["Shift+Pause", "Ctrl+Alt+K"]);
    assert!(
        !s.iter().any(|x| x.eq_ignore_ascii_case("Ctrl+Alt+K")),
        "the other LexiSwitch shortcut was suggested: {s:?}"
    );
    assert!(!s.is_empty(), "excluding two should not empty the list");
}

/// Case and stray spacing in the configuration file should not defeat the filter.
#[test]
fn suggestions_match_the_current_shortcut_loosely() {
    for current in ["ctrl+alt+k", "  Ctrl+Alt+K  "] {
        let s = lexiswitch_core::state::suggested_shortcuts(&[current]);
        assert!(!s.iter().any(|x| x.eq_ignore_ascii_case("ctrl+alt+k")), "{s:?}");
    }
}

/// Short enough to read at a glance; a list of ten is not a suggestion.
#[test]
fn suggestions_are_few() {
    assert!(lexiswitch_core::state::suggested_shortcuts(&["Shift+Pause"]).len() <= 3);
}

// --- three shortcuts -----------------------------------------------------------------

#[test]
fn the_summary_names_all_three_working_shortcuts() {
    let summary = AgentState::resolve(2, all_registered(), true, None).summary();
    for binding in ["Shift+Pause", "Win+Ctrl+Pause", "Win+Alt+Pause"] {
        assert!(summary.contains(binding), "{binding} missing from {summary}");
    }
    let explanation = AgentState::resolve(2, all_registered(), true, None).explanation();
    assert!(explanation.contains("Win+Alt+Pause"), "{explanation}");
}

#[test]
fn three_long_conflicts_still_fit_a_tooltip() {
    let s = AgentState::resolve(
        3,
        Shortcuts {
            convert: Shortcut::Conflict("Ctrl+Alt+Shift+Insert".into()),
            case: Shortcut::Conflict("Ctrl+Alt+Shift+PageDown".into()),
            translit: Shortcut::Conflict("Ctrl+Alt+Shift+PageUp".into()),
        },
        true,
        None,
    );
    let n = s.summary().chars().count();
    assert!(n <= 127, "{n} characters is too long for a tooltip: {}", s.summary());
}

// --- a default that gave way ---------------------------------------------------------

/// The user's configuration bound case to what is now transliteration's default.
fn translit_gave_way() -> Shortcuts {
    Shortcuts {
        convert: Shortcut::Registered("Win+Shift+Pause".into()),
        case: Shortcut::Registered("Alt+Win+Pause".into()),
        translit: Shortcut::Unbound {
            binding: "Win+Alt+Pause".into(),
            taken_by: "change_case",
        },
    }
}

/// A shortcut with no binding needs the user, but it is not another application's doing and
/// must not be reported as though it were.
#[test]
fn an_unbound_shortcut_needs_attention_but_is_not_a_conflict() {
    let s = AgentState::resolve(3, translit_gave_way(), true, None);
    assert!(matches!(s, AgentState::ShortcutUnbound { .. }), "{s:?}");
    assert!(s.needs_attention());
    assert!(s.converts());
    assert!(!s.explanation().contains("another application"), "{}", s.explanation());
}

/// What would have been bound, what holds it, and what still works — all three are needed to
/// act on it.
#[test]
fn an_unbound_shortcut_says_what_holds_its_default() {
    let s = AgentState::resolve(3, translit_gave_way(), true, None);
    let explanation = s.explanation();
    assert!(explanation.contains("Win+Alt+Pause"), "{explanation}");
    assert!(explanation.contains("changes case"), "{explanation}");
    assert!(explanation.contains("transliterat"), "{explanation}");
    assert!(explanation.contains("Win+Shift+Pause"), "the working shortcuts are not named: {explanation}");
    assert!(s.summary().contains("no shortcut"), "{}", s.summary());
    assert!(s.summary().chars().count() <= 127);
}

/// Another application holding a shortcut outranks a shortcut that gave way: the conflict is the
/// one the user cannot see the cause of.
#[test]
fn a_conflict_outranks_an_unbound_shortcut() {
    let mut shortcuts = translit_gave_way();
    shortcuts.convert = Shortcut::Conflict("Win+Shift+Pause".into());
    let s = AgentState::resolve(3, shortcuts, true, None);
    assert!(matches!(s, AgentState::ShortcutConflict { .. }), "{s:?}");
}

#[test]
fn being_disabled_does_not_hide_an_unbound_shortcut() {
    let s = AgentState::resolve(3, translit_gave_way(), false, None);
    assert!(matches!(s, AgentState::ShortcutUnbound { .. }), "{s:?}");
}

#[test]
fn suggestions_exclude_all_three_shortcuts() {
    let held = ["Ctrl+Alt+K", "Alt+Pause", "Ctrl+Alt+Space"];
    let s = lexiswitch_core::state::suggested_shortcuts(&held);
    for h in held {
        assert!(!s.iter().any(|x| x.eq_ignore_ascii_case(h)), "{h} suggested: {s:?}");
    }
    assert!(!s.is_empty());
}

// --- rulesets that did not load ------------------------------------------------------

/// A rejected ruleset leaves every shortcut working and every other ruleset in use, but the
/// user has a file to fix, so the tray says so.
#[test]
fn a_rejected_ruleset_needs_attention_and_still_converts() {
    let s = AgentState::resolve(3, all_registered(), true, None)
        .with_rejected_rulesets(vec!["mine.toml: line 4: unknown field `wordstart`".into()]);
    assert!(matches!(s, AgentState::RulesetRejected { .. }), "{s:?}");
    assert!(s.needs_attention());
    assert!(s.converts());
    assert!(s.explanation().contains("mine.toml: line 4"), "{}", s.explanation());
    assert!(s.explanation().contains("Win+Alt+Pause"), "the shortcuts still work: {}", s.explanation());
    assert!(s.summary().chars().count() <= 127);
}

#[test]
fn no_rejected_ruleset_changes_nothing() {
    let s = AgentState::resolve(3, all_registered(), true, None);
    assert_eq!(s.clone().with_rejected_rulesets(Vec::new()), s);
}

/// Anything more blocking keeps its place: a rejected ruleset is the least of the problems.
#[test]
fn a_rejected_ruleset_does_not_mask_a_worse_state() {
    let rejected = || vec!["x.toml: bad".to_string()];
    let few = AgentState::resolve(1, all_registered(), true, None).with_rejected_rulesets(rejected());
    assert_eq!(few, AgentState::TooFewLayouts { installed: 1 });
    let off = AgentState::resolve(3, all_registered(), false, None).with_rejected_rulesets(rejected());
    assert_eq!(off, AgentState::Disabled);
}
