//! What the agent can currently do, and what it should say about it.
//!
//! The program had two ways of ending before it started: a shortcut another application already
//! held, and fewer than two installed keyboard layouts. Both were `?` at startup, so the user
//! got an exit code and no tray icon — which looks exactly like the program being broken. It
//! happened during this project's own testing, when a stale instance held the hotkey and a
//! perfectly good build appeared to be dead.
//!
//! Neither condition is unrecoverable. A shortcut can be re-registered, a layout can be
//! installed, and in both cases the user can act if they are told what is wrong. So both become
//! states the program carries while it keeps running, and the tray is where they are shown.
//!
//! This module decides which state holds and what it means. It knows nothing about Windows,
//! which is what lets the precedence rules be tested without one.

/// Whether one invocation shortcut is bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shortcut {
    /// Registered, and this is the combination.
    Registered(String),
    /// Another application holds it. The combination is carried so it can be named — "the
    /// shortcut is taken" is not something a user can act on without knowing which one.
    Conflict(String),
    /// Never attempted: the shortcut was left at its default, and the user's configuration binds
    /// that combination to another shortcut, which wins. `taken_by` is that other shortcut's
    /// configuration key ([ADR-0017](../../docs/adr/0017-transliteration-third-transaction.md)).
    Unbound {
        binding: String,
        taken_by: &'static str,
    },
}

impl Shortcut {
    /// The combination this shortcut is, or for an unbound one would have been.
    pub fn binding(&self) -> &str {
        match self {
            Self::Registered(b) | Self::Conflict(b) | Self::Unbound { binding: b, .. } => b,
        }
    }

    fn registered(&self) -> bool {
        matches!(self, Self::Registered(_))
    }
}

/// Every shortcut the agent wants, and how each one fared.
///
/// There is one per transaction: conversion, case change and transliteration are separate
/// invocations ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md),
/// [ADR-0017](../../docs/adr/0017-transliteration-third-transaction.md)). They are carried
/// together because a conflict is not a single fact — the user needs to know which combination
/// is unavailable *and* which still work, and neither half is useful without the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcuts {
    pub convert: Shortcut,
    pub case: Shortcut,
    pub translit: Shortcut,
}

impl Shortcuts {
    /// Each shortcut with a phrase describing what it does, in the order they are shown.
    ///
    /// The phrases live here rather than at the point of formatting so that every message about
    /// a shortcut describes it the same way.
    fn described(&self) -> [(&Shortcut, &'static str); 3] {
        [
            (&self.convert, describe("convert_selection")),
            (&self.case, describe("change_case")),
            (&self.translit, describe("transliterate")),
        ]
    }
}

/// What the shortcut under a `[hotkeys]` key does, as the messages phrase it.
fn describe(key: &str) -> &'static str {
    match key {
        "convert_selection" => "converts the selection",
        "change_case" => "changes case",
        "transliterate" => "transliterates",
        _ => "is another shortcut",
    }
}


/// The condition the agent is in.
///
/// Deliberately flat rather than a set of independent flags. Several conditions can hold at
/// once, and the user needs to be pointed at the one that blocks the most — being told about a
/// shortcut conflict while only one layout is installed would send them to fix the wrong thing,
/// and leave them with a working shortcut that still converts nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// Everything works. Carries every registered combination, because the tray tooltip is the
    /// only place any of them is visible without opening the configuration file.
    Enabled {
        convert: String,
        case: String,
        translit: String,
    },
    /// Switched off by the user. Not a problem, and not something to nag about.
    Disabled,
    /// Fewer than two layouts: there is nothing to convert into.
    TooFewLayouts { installed: usize },
    /// At least one shortcut is held elsewhere. Both transactions still work from the tray.
    ///
    /// Carries what did *not* register and what did, each with a phrase describing it. Naming
    /// only the failure would leave the user unable to tell a half-working agent from a dead
    /// one — and with two shortcuts, half-working is the likeliest kind of conflict there is.
    ShortcutConflict {
        unavailable: Vec<(String, &'static str)>,
        working: Vec<(String, &'static str)>,
    },
    /// At least one shortcut gave its default up to a binding the user wrote, so it has no
    /// combination at all. Nothing is held elsewhere; the user only has to choose one.
    ///
    /// `unbound` carries, per shortcut, the transaction it opens, the combination it would have
    /// had, and what that combination does instead.
    ShortcutUnbound {
        unbound: Vec<(&'static str, String, &'static str)>,
        working: Vec<(String, &'static str)>,
    },
    /// Everything works, but at least one transliteration ruleset file was not loaded. Its last
    /// good version, if it had one, is in use, as is every other ruleset. Carries what `Enabled`
    /// does and why each file was rejected — the file, the line and the problem, all from the
    /// user's own ruleset, none of it from selected text.
    RulesetRejected {
        convert: String,
        case: String,
        translit: String,
        rejected: Vec<String>,
    },
    /// Something failed at runtime and nothing else can be relied on.
    RuntimeError { detail: String },
}

impl AgentState {
    /// Work out the state from the conditions that hold.
    ///
    /// The order of the checks *is* the precedence, most blocking first.
    pub fn resolve(
        layouts: usize,
        shortcuts: Shortcuts,
        enabled: bool,
        error: Option<String>,
    ) -> Self {
        // A runtime error first: while one holds, no other condition can be trusted to have
        // been established correctly.
        if let Some(detail) = error {
            return Self::RuntimeError { detail };
        }
        // Then the one that makes conversion impossible rather than merely unbound.
        if layouts < 2 {
            return Self::TooFewLayouts { installed: layouts };
        }
        // Ahead of `enabled`, so switching off does not hide something the user must act on.
        // Otherwise turning it back on later would produce a surprise instead of a conversion.
        let described = shortcuts.described();
        // A conflict first: an unbound shortcut is explained by the user's own file, a held one
        // by something the user cannot see. With both, the unbound one is left out of `working`
        // and reported once the conflict is resolved.
        if described.iter().any(|(s, _)| matches!(s, Shortcut::Conflict(_))) {
            let (working, unavailable): (Vec<_>, Vec<_>) = described
                .iter()
                .filter(|(s, _)| !matches!(s, Shortcut::Unbound { .. }))
                .map(|(s, what)| (s.binding().to_string(), *what, s.registered()))
                .partition(|(_, _, ok)| *ok);
            let strip = |v: Vec<(String, &'static str, bool)>| {
                v.into_iter().map(|(b, w, _)| (b, w)).collect::<Vec<_>>()
            };
            return Self::ShortcutConflict {
                unavailable: strip(unavailable),
                working: strip(working),
            };
        }
        if described.iter().any(|(s, _)| !s.registered()) {
            let unbound = described
                .iter()
                .filter_map(|(s, what)| match s {
                    Shortcut::Unbound { binding, taken_by } => {
                        Some((*what, binding.clone(), describe(taken_by)))
                    }
                    _ => None,
                })
                .collect();
            let working = described
                .iter()
                .filter(|(s, _)| s.registered())
                .map(|(s, what)| (s.binding().to_string(), *what))
                .collect();
            return Self::ShortcutUnbound { unbound, working };
        }
        if !enabled {
            return Self::Disabled;
        }
        Self::Enabled {
            convert: shortcuts.convert.binding().to_string(),
            case: shortcuts.case.binding().to_string(),
            translit: shortcuts.translit.binding().to_string(),
        }
    }

    /// Refine a resolved state with the transliteration rulesets that failed to load.
    ///
    /// A separate step rather than another argument to [`AgentState::resolve`], because it only
    /// ever refines the healthy state: anything more blocking keeps its place, and being
    /// disabled is still the user's choice to be told about first.
    pub fn with_rejected_rulesets(self, rejected: Vec<String>) -> Self {
        match self {
            Self::Enabled {
                convert,
                case,
                translit,
            } if !rejected.is_empty() => Self::RulesetRejected {
                convert,
                case,
                translit,
                rejected,
            },
            other => other,
        }
    }

    /// Whether a conversion request should be acted on.
    ///
    /// True during a shortcut conflict. The shortcut is what is unavailable, not the
    /// conversion — the tray command still works, and that is the whole reason it exists. A
    /// conflict that also disabled conversion would leave a running program that can do
    /// nothing at all.
    pub fn converts(&self) -> bool {
        matches!(
            self,
            Self::Enabled { .. }
                | Self::ShortcutConflict { .. }
                | Self::ShortcutUnbound { .. }
                | Self::RulesetRejected { .. }
        )
    }

    /// Whether every invocation shortcut is actually bound.
    pub fn shortcut_works(&self) -> bool {
        !matches!(
            self,
            Self::ShortcutConflict { .. } | Self::ShortcutUnbound { .. }
        )
    }

    /// Whether the user has to do something about this.
    ///
    /// False for `Disabled`: it is a choice, not a fault, and presenting it as one would train
    /// the user to ignore the indicator that matters.
    pub fn needs_attention(&self) -> bool {
        matches!(
            self,
            Self::TooFewLayouts { .. }
                | Self::ShortcutConflict { .. }
                | Self::ShortcutUnbound { .. }
                | Self::RulesetRejected { .. }
                | Self::RuntimeError { .. }
        )
    }

    /// One line, for the tray tooltip.
    ///
    /// Names the shortcut whenever there is a working one. A tray icon that does not say how to
    /// invoke the program teaches nothing, and this is the only place the shortcut is visible
    /// without opening the configuration file.
    pub fn summary(&self) -> String {
        match self {
            Self::Enabled {
                convert,
                case,
                translit,
            } => format!(
                "LexiSwitch — {convert} converts, {case} changes case, {translit} transliterates"
            ),
            Self::Disabled => "LexiSwitch — disabled".to_string(),
            Self::TooFewLayouts { installed } => {
                format!("LexiSwitch — {installed} keyboard layout installed, two are needed")
            }
            // Windows cuts a tooltip off at 128 characters. Three long combinations with what
            // each does would not fit, and the combinations are the part the user can act on.
            Self::ShortcutConflict { unavailable, .. } => {
                let full = format!("LexiSwitch — {} unavailable", join(unavailable));
                if full.chars().count() <= 127 {
                    return full;
                }
                let bindings: Vec<&str> = unavailable.iter().map(|(b, _)| b.as_str()).collect();
                format!("LexiSwitch — {} unavailable", bindings.join(", "))
            }
            Self::ShortcutUnbound { unbound, .. } => {
                let names: Vec<&str> = unbound.iter().map(|(what, ..)| *what).collect();
                format!("LexiSwitch — no shortcut that {}", names.join(" or "))
            }
            Self::RulesetRejected { rejected, .. } => format!(
                "LexiSwitch — {} transliteration ruleset{} not loaded",
                rejected.len(),
                if rejected.len() == 1 { "" } else { "s" }
            ),
            Self::RuntimeError { .. } => "LexiSwitch — something went wrong".to_string(),
        }
    }

    /// The same state at length, for a balloon or a settings panel.
    pub fn explanation(&self) -> String {
        match self {
            Self::Enabled {
                convert,
                case,
                translit,
            } => format!(
                "Select text and press {convert} to convert it, {case} to change its case, or \
                 {translit} to transliterate it."
            ),
            Self::Disabled => {
                "The shortcuts do not convert while LexiSwitch is disabled. Nothing is being \
                 recorded either way."
                    .to_string()
            }
            Self::TooFewLayouts { installed } => format!(
                "Conversion needs at least two keyboard layouts, and {installed} is installed. \
                 Add a second layout in Windows settings; LexiSwitch will pick it up without \
                 being restarted."
            ),
            // Three sentences, and the middle one is the important one: which shortcut survived.
            // Without it a conflict on either shortcut reads as the whole program being
            // unavailable, and the user goes looking for a fault that is not there.
            Self::ShortcutConflict {
                unavailable,
                working,
            } => {
                let mut out = format!(
                    "{} already held by another application, so LexiSwitch did not take {}. ",
                    join(unavailable),
                    if unavailable.len() == 1 { "it" } else { "them" },
                );
                if !working.is_empty() {
                    out.push_str(&format!("{} still working. ", join(working)));
                }
                out.push_str(
                    "Every command remains available from the tray menu. Choose a different \
                     shortcut to use the keyboard instead.",
                );
                out
            }
            // Not "held by another application": the user's own file holds it, and the fix is to
            // choose a combination, not to hunt for a culprit.
            Self::ShortcutUnbound { unbound, working } => {
                let mut out = String::new();
                for (what, binding, taken_by) in unbound {
                    out.push_str(&format!(
                        "The shortcut that {what} has none: its default, {binding}, already \
                         {taken_by} in your configuration. "
                    ));
                }
                if !working.is_empty() {
                    out.push_str(&format!("{} working. ", join(working)));
                }
                out.push_str(
                    "Every command remains available from the tray menu, where you can also \
                     choose a shortcut for it.",
                );
                out
            }
            Self::RulesetRejected {
                convert,
                case,
                translit,
                rejected,
            } => format!(
                "Not loaded: {}. Every other ruleset is in use. Select text and press {convert} to \
                 convert it, {case} to change its case, or {translit} to transliterate it.",
                rejected.join("; ")
            ),
            Self::RuntimeError { detail } => format!("LexiSwitch hit a problem: {detail}"),
        }
    }
}

/// Name some shortcuts and what they do, as one readable phrase.
///
/// Renders as `"Shift+Pause (converts the selection) is"` or `"A (…) and B (…) are"` — the verb
/// is included because the caller cannot know how many there will be, and "1 shortcuts are"
/// is the kind of detail that makes a message look machine-written.
fn join(items: &[(String, &'static str)]) -> String {
    let named: Vec<String> = items
        .iter()
        .map(|(binding, what)| format!("{binding} ({what})"))
        .collect();
    match named.len() {
        0 => "nothing is".to_string(),
        1 => format!("{} is", named[0]),
        _ => format!("{} are", named.join(" and ")),
    }
}

/// A few combinations worth trying, when the configured one is unavailable.
///
/// Suggestions, and nothing stronger. Whether any of these is free depends on every other
/// application running on the machine, which cannot be known from here — the only way to find
/// out is to attempt the registration, which is what the recorder does. Presenting them as
/// guaranteed would be a promise this program is in no position to make, and the one thing
/// worse than a conflict is being told a combination is free and finding it is not.
///
/// The list is deliberately short and unglamorous: combinations unlikely to collide with an
/// editor's own bindings, with the current one filtered out because suggesting what the user
/// already has is noise.
pub fn suggested_shortcuts(taken: &[&str]) -> Vec<String> {
    ["Ctrl+Alt+K", "Ctrl+Shift+Insert", "Alt+Pause", "Ctrl+Alt+Space"]
        .into_iter()
        .filter(|s| {
            // Every combination the program already holds, not only the one being replaced.
            // Suggesting the other shortcut offers something certain to be refused, and the
            // refusal names LexiSwitch itself — which reads as a fault rather than as advice.
            !taken.iter().any(|t| s.eq_ignore_ascii_case(t.trim()))
        })
        .take(3)
        .map(str::to_string)
        .collect()
}
