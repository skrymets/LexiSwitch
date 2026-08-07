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

/// Whether the invocation shortcut is bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shortcut {
    /// Registered, and this is the combination.
    Registered(String),
    /// Another application holds it. The combination is carried so it can be named — "the
    /// shortcut is taken" is not something a user can act on without knowing which one.
    Conflict(String),
}

impl Shortcut {
    fn binding(&self) -> &str {
        match self {
            Self::Registered(b) | Self::Conflict(b) => b,
        }
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
    /// Everything works. Carries the registered combination, because the tray tooltip is the
    /// only place the shortcut is visible without opening the configuration file.
    Enabled { binding: String },
    /// Switched off by the user. Not a problem, and not something to nag about.
    Disabled,
    /// Fewer than two layouts: there is nothing to convert into.
    TooFewLayouts { installed: usize },
    /// The shortcut is held elsewhere. Conversion still works from the tray.
    ShortcutConflict { binding: String },
    /// Something failed at runtime and nothing else can be relied on.
    RuntimeError { detail: String },
}

impl AgentState {
    /// Work out the state from the conditions that hold.
    ///
    /// The order of the checks *is* the precedence, most blocking first.
    pub fn resolve(
        layouts: usize,
        shortcut: Shortcut,
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
        if let Shortcut::Conflict(binding) = &shortcut {
            return Self::ShortcutConflict {
                binding: binding.clone(),
            };
        }
        if !enabled {
            return Self::Disabled;
        }
        Self::Enabled {
            binding: shortcut.binding().to_string(),
        }
    }

    /// Whether a conversion request should be acted on.
    ///
    /// True during a shortcut conflict. The shortcut is what is unavailable, not the
    /// conversion — the tray command still works, and that is the whole reason it exists. A
    /// conflict that also disabled conversion would leave a running program that can do
    /// nothing at all.
    pub fn converts(&self) -> bool {
        matches!(self, Self::Enabled { .. } | Self::ShortcutConflict { .. })
    }

    /// Whether the invocation shortcut is actually bound.
    pub fn shortcut_works(&self) -> bool {
        !matches!(self, Self::ShortcutConflict { .. })
    }

    /// Whether the user has to do something about this.
    ///
    /// False for `Disabled`: it is a choice, not a fault, and presenting it as one would train
    /// the user to ignore the indicator that matters.
    pub fn needs_attention(&self) -> bool {
        matches!(
            self,
            Self::TooFewLayouts { .. } | Self::ShortcutConflict { .. } | Self::RuntimeError { .. }
        )
    }

    /// One line, for the tray tooltip.
    ///
    /// Names the shortcut whenever there is a working one. A tray icon that does not say how to
    /// invoke the program teaches nothing, and this is the only place the shortcut is visible
    /// without opening the configuration file.
    pub fn summary(&self) -> String {
        match self {
            Self::Enabled { binding } => format!("LexiSwitch — {binding} converts the selection"),
            Self::Disabled => "LexiSwitch — disabled".to_string(),
            Self::TooFewLayouts { installed } => {
                format!("LexiSwitch — {installed} keyboard layout installed, two are needed")
            }
            Self::ShortcutConflict { binding } => {
                format!("LexiSwitch — {binding} is unavailable")
            }
            Self::RuntimeError { .. } => "LexiSwitch — something went wrong".to_string(),
        }
    }

    /// The same state at length, for a balloon or a settings panel.
    pub fn explanation(&self) -> String {
        match self {
            Self::Enabled { binding } => {
                format!("Select text and press {binding} to convert it.")
            }
            Self::Disabled => {
                "The shortcut does not convert while LexiSwitch is disabled. Nothing is being \
                 recorded either way."
                    .to_string()
            }
            Self::TooFewLayouts { installed } => format!(
                "Conversion needs at least two keyboard layouts, and {installed} is installed. \
                 Add a second layout in Windows settings; LexiSwitch will pick it up without \
                 being restarted."
            ),
            Self::ShortcutConflict { binding } => format!(
                "{binding} is already held by another application, so LexiSwitch did not take \
                 it. Conversion still works from the tray menu. Choose a different shortcut to \
                 use the keyboard instead."
            ),
            Self::RuntimeError { detail } => format!("LexiSwitch hit a problem: {detail}"),
        }
    }
}
