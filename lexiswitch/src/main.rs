//! The binary: wiring, configuration, and the event loop. Logic lives in `lexiswitch-core`,
//! Win32 in `lexiswitch-win`; this crate holds neither.
//!
//! Two threads, split by what is allowed to be slow. This one runs the window, hotkeys and
//! the tray, and never blocks. Anything that can stall — UI Automation, the clipboard, input
//! synthesis — happens on the worker, reached by sending it a snapshot.
//!
//! Nothing here observes typing. The program acts only on text the user has selected, and
//! only when the hotkey is pressed ([ADR-0011](../../docs/adr/0011-no-keyboard-observation.md)).

// This crate wires two crates together; it has no reason to dereference a raw pointer.
#![forbid(unsafe_code)]
// No console window in the shipped build — LexiSwitch is a tray agent. Left to the release
// profile only, so a debug run still has somewhere to print.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::sync::Arc;

use lexiswitch_core::config::Config;
use lexiswitch_core::state::{AgentState, Shortcut, Shortcuts};
use lexiswitch_core::translit::{RulesetFailure, Rulesets};

use lexiswitch_win::focus::process_name;
use lexiswitch_win::hotkey::{parse_binding, HotkeyId, Hotkeys};
use lexiswitch_win::recorder::{self, Recorded};
use lexiswitch_win::inject::FocusGuard;
use lexiswitch_win::instance;
use lexiswitch_win::layouts::{self, InstalledLayout};
use lexiswitch_win::tray::{tray_event, MenuCommand, Tray, WM_TRAY};
use lexiswitch_win::window::{self, Flow, MessageWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_COMMAND, WM_CONTEXTMENU, WM_HOTKEY, WM_RBUTTONUP,
};

mod configwatch;
mod worker;

use configwatch::{ConfigWatcher, EventLog, Loaded, RulesetWatcher};
use worker::{Job, Outcome, Worker};

/// The version, from the workspace manifest — the one place it is declared (ADR-0016). The
/// release, the file's version resource and this are the same string by construction.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What heads the tray menu.
const TITLE: &str = concat!("LexiSwitch ", env!("CARGO_PKG_VERSION"));

fn main() {
    if let Err(e) = run() {
        // Startup failures happen before the tray exists, so they have nowhere else to go.
        eprintln!("lexiswitch: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    // --- one agent per session ------------------------------------------------------
    //
    // First, before configuration is read, before layouts are built and before the shortcut is
    // claimed. A second instance that got as far as any of those would already have written a
    // log line, taken a tray icon, or reported a conflict with the instance it is about to
    // decline to replace.
    let _instance = match instance::acquire() {
        instance::Instance::First(guard) => guard,
        instance::Instance::AlreadyRunning => {
            // Not silence: the usual reason to start it twice is having lost the first one, so
            // the running instance is asked to say where it is. Not an error either — the user
            // wanted LexiSwitch running, and it is.
            let shown = window::ask_running_instance_to_show();
            eprintln!(
                "lexiswitch: already running{}",
                if shown {
                    "; the running one has been asked to show itself"
                } else {
                    ", but it could not be reached to show itself"
                }
            );
            return Ok(());
        }
    };

    // --- configuration -------------------------------------------------------------
    let config_path = configwatch::default_path();
    // Where the case window's usage counts live. Resolved here and handed to the worker, which
    // owns the file outright (ADR-0015).
    let state_file = configwatch::state_path();
    let mut config = match &config_path {
        Some(path) => match configwatch::load(path) {
            Loaded::Ok(config) => *config,
            Loaded::Missing => Config::default(),
            Loaded::Invalid(why) => {
                // Startup with a broken file uses defaults rather than refusing to run, and
                // says so. A tool that will not start because of a typo is worse than one
                // running known-good settings.
                eprintln!("lexiswitch: {why}; starting with defaults");
                Config::default()
            }
        },
        None => Config::default(),
    };
    let mut log = EventLog::new(config.logging.enabled);

    // Set when something fails in a way that leaves nothing else trustworthy. Carried rather
    // than returned, so the tray still comes up to report it.
    let mut runtime_error: Option<String> = None;

    // --- transliteration rulesets ---------------------------------------------------
    //
    // The built-ins overlaid by the user's folder, compiled now and immutable afterwards. A file
    // that fails is reported once the tray exists; every other ruleset is used regardless.
    // Shared with the worker by `Arc`, so a reload swaps this copy and leaves one already handed
    // to an open window alone.
    let (loaded, mut ruleset_failures) =
        configwatch::load_rulesets(configwatch::translit_dir().as_deref(), &Rulesets::builtin());
    let mut rulesets = Arc::new(loaded);

    // Reported, not silently dropped. A removed setting and a misspelled one both land here,
    // and naming them is the only way the user can tell which they are looking at. Held until
    // the tray exists, since that is where the user will see it.
    let ignored_at_startup = config.ignored_keys();

    // --- layouts, window, hotkeys ---------------------------------------------------
    //
    // Neither of the two conditions below ends the process any more. Both used to be `?`, so a
    // user whose shortcut was already taken — or who had one layout installed — got an exit
    // code and no tray icon, which is indistinguishable from the program being broken. Both
    // are recoverable without a restart, and neither can be acted on by someone who was never
    // told about it.
    let (mut layouts, mut layout_count) = match layouts::build_all() {
        Ok(built) => {
            let n = built.len();
            log.note(&format!("layouts_built count={n}"));
            (built, n)
        }
        Err(layouts::LayoutError::TooFewLayouts(n)) => {
            log.note(&format!("layouts_insufficient count={n}"));
            (Vec::new(), n)
        }
        // Anything else really is a failure to build tables, not a shortage of them.
        Err(e) => {
            log.note("layouts_failed");
            runtime_error = Some(e.to_string());
            (Vec::new(), 0)
        }
    };

    let window = MessageWindow::new()?;

    // A shortcut another application holds is a state, not an exit — and now that there are
    // two, one being held is not a statement about the other. Registration is per binding, so
    // a conflict on the case shortcut leaves conversion working and vice versa.
    let (mut hotkeys, mut shortcuts) = register_hotkeys(&window, &config, &mut log);

    let mut enabled = config.general.enabled;
    let mut state = resolve_state(
        layout_count,
        &shortcuts,
        enabled,
        &runtime_error,
        &ruleset_failures,
    );

    let tray = Tray::new(window.hwnd(), &state)?;
    // Whatever the state is, say it — including when it is simply working. The startup balloon
    // is where the shortcut is learned.
    tray.notify("LexiSwitch", &state.explanation());
    if !ignored_at_startup.is_empty() {
        tray.notify(
            "Some settings were ignored",
            &format!(
                "{} is not a setting this version has. It was ignored and everything else was \
                 applied. Check the spelling if you did not expect this.",
                ignored_at_startup.join(", ")
            ),
        );
    }

    // --- the worker ----------------------------------------------------------------
    let (work, outcomes) = Worker::spawn(window.waker());
    let mut watcher = config_path.clone().map(ConfigWatcher::new);
    let mut ruleset_watcher = RulesetWatcher::new(configwatch::translit_dir());

    // A heartbeat, so the configuration file is actually polled. The poll lives in the handler
    // below, and the handler only runs when a message arrives — an idle agent gets none, so
    // without this an edit to the file is noticed only when something else happens to wake the
    // loop. Two seconds is the interval the file watching was always documented as having.
    if !window.start_tick(std::time::Duration::from_secs(2)) {
        log.note("tick_not_started");
    }

    window.run(|msg| {
        // Anything the worker finished gets reported here, on the thread that owns the tray.
        while let Ok(outcome) = outcomes.try_recv() {
            match outcome {
                Outcome::Quiet { note } => log.note(note),
                Outcome::Note { title, body } => {
                    log.note("outcome_note");
                    tray.notify(&title, &body);
                }
                Outcome::Failed { title, body } => {
                    // Also to the log, because the tray is not a reliable channel. Balloons
                    // were observed not displaying at all on this machine, and a failure the
                    // user cannot see is the silent failure the spec forbids. Titles and
                    // bodies here are this program's own strings — no selected text reaches
                    // them, so logging them does not weaken the no-content rule.
                    log.note(&format!("outcome_failed: {title}: {body}"));
                    tray.notify(&title, &body);
                }
            }
        }

        // Hot reload (task 9.5). Polling costs a metadata read every two seconds.
        if let Some(w) = watcher.as_mut() {
            if w.changed() {
                match configwatch::load(w.path()) {
                    Loaded::Ok(new) => {
                        let new = *new;
                        // Swapped wholesale: nothing reads half-old, half-new state.
                        let rebind = new.hotkeys != config.hotkeys;
                        config = new;
                        log.set_enabled(config.logging.enabled);
                        if rebind {
                            // Release the old bindings before claiming the new ones, or the
                            // unchanged ones collide with themselves.
                            hotkeys = None;
                            let (h, s) = register_hotkeys(&window, &config, &mut log);
                            hotkeys = h;
                            shortcuts = s;
                            // Only what failed, and only when something did. A reload that
                            // rebinds successfully already reports itself below.
                            let refused = unavailable(&shortcuts);
                            if !refused.is_empty() {
                                tray.notify(
                                    "Some shortcuts were not registered",
                                    &format!(
                                        "{refused} Every command remains available from this \
                                         menu."
                                    ),
                                );
                            }
                        }
                        let ignored = config.ignored_keys();
                        if ignored.is_empty() {
                            tray.notify(
                                "Configuration reloaded",
                                "The new settings are in effect.",
                            );
                        } else {
                            tray.notify(
                                "Configuration reloaded, some settings ignored",
                                &format!("Applied, except: {}.", ignored.join(", ")),
                            );
                        }
                        log.note("config_reloaded");
                    }
                    Loaded::Missing => {
                        config = Config::default();
                        tray.notify(
                            "Configuration removed",
                            "The file is gone; default settings are in effect.",
                        );
                    }
                    Loaded::Invalid(why) => {
                        // Not swapped in. Last-known-good stays, which is the whole point of
                        // validating before replacing.
                        tray.notify("Configuration not applied", &why);
                        log.note("config_rejected");
                    }
                }
                // The configuration decides which shortcuts are bound, and which gave way.
                state = resolve_state(
                    layout_count,
                    &shortcuts,
                    enabled,
                    &runtime_error,
                    &ruleset_failures,
                );
                tray.set_state(&state);
            }
        }

        // The rulesets folder, on the same tick. Rebuilt whole and swapped whole; a file that
        // fails keeps its last good version, which is why the current set is passed in.
        if ruleset_watcher.changed() {
            let (loaded, failures) = configwatch::load_rulesets(ruleset_watcher.dir(), &rulesets);
            rulesets = Arc::new(loaded);
            log.note(&format!(
                "rulesets_reloaded count={} rejected={}",
                rulesets.len(),
                failures.len()
            ));
            if failures.is_empty() {
                tray.notify(
                    "Transliteration rulesets reloaded",
                    &format!("{} rulesets are offered.", rulesets.len()),
                );
            } else {
                tray.notify("Transliteration ruleset not loaded", &describe_failures(&failures));
            }
            ruleset_failures = failures;
            state = resolve_state(
                layout_count,
                &shortcuts,
                enabled,
                &runtime_error,
                &ruleset_failures,
            );
            tray.set_state(&state);
        }

        // Which menu command to act on, if any, from whichever route it arrived by.
        //
        // Two routes, one handler. `TrackPopupMenu` returns the chosen id directly, while
        // `WM_COMMAND` is how a menu selection arrives by every other means — and it is the
        // only route reachable from outside the process, which is what makes these commands
        // testable at all. Quit was already handled twice; the rest were handled once, so a
        // posted Convert selection did nothing and looked exactly like a broken feature.
        let mut command = None;

        match msg.message {
            WM_HOTKEY => {
                // Every shortcut is gated on the same enabled state: disabling LexiSwitch
                // disables LexiSwitch, not one of its transactions.
                if state.converts()
                    && matches!(
                        HotkeyId::from_raw(msg.wParam.0 as i32),
                        Some(HotkeyId::Transliterate)
                    )
                {
                    request_translit(&work, &rulesets, &config, &mut log);
                }
                if state.converts()
                    && matches!(
                        HotkeyId::from_raw(msg.wParam.0 as i32),
                        Some(HotkeyId::ChangeCase)
                    )
                {
                    request_case(&work, &config, &state_file, &mut log);
                }
                if state.converts()
                    && matches!(
                        HotkeyId::from_raw(msg.wParam.0 as i32),
                        Some(HotkeyId::ConvertSelection)
                    )
                {
                    // Every installed layout goes across, not a chosen pair. Which conversions
                    // are plausible is decided from the selected characters; the active layout
                    // travels only as a weak tiebreak on the target, because it is a fact about
                    // the present and says nothing about text typed at any earlier time
                    // (ADR-0012).
                    request_conversion(&work, &layouts, &config, &mut log);
                }
            }
            // Logged unconditionally, before any guard. Two rounds of fixing this menu have
            // assumed the message arrives and only the handling was wrong; nothing has yet
            // shown that it arrives at all. `lparam` carries the mouse message under the
            // legacy tray protocol this icon uses.
            WM_TRAY => {
                let click = tray_event(msg.lParam.0);
                log.note(&format!("tray_message event=0x{click:04X}"));

                // Recover from the two conditions that are outside this program's control, by
                // re-checking them rather than requiring a restart. Both are cheap and both are
                // only interesting while they hold: a layout can be installed and a shortcut
                // can be released by whatever was holding it, and in neither case does anything
                // notify us.
                if layout_count < 2 {
                    if let Ok(built) = layouts::build_all() {
                        log.note(&format!("layouts_appeared count={}", built.len()));
                        layout_count = built.len();
                        layouts = built;
                    }
                }
                if [&shortcuts.convert, &shortcuts.case, &shortcuts.translit]
                    .iter()
                    .any(|s| matches!(s, Shortcut::Conflict(_)))
                {
                    // Release first: re-registering a combination this process already holds
                    // would be refused, and would look exactly like the conflict it is meant
                    // to clear.
                    hotkeys = None;
                    let (h, s) = register_hotkeys(&window, &config, &mut log);
                    hotkeys = h;
                    // Named per shortcut, because with two of them "the conflict cleared" is
                    // no longer one fact. Identifiers only — the combination itself comes from
                    // the user's file and does not need to be in a log to make this useful.
                    for (was, now, which) in [
                        (&shortcuts.convert, &s.convert, "convert"),
                        (&shortcuts.case, &s.case, "case"),
                        (&shortcuts.translit, &s.translit, "translit"),
                    ] {
                        if matches!(was, Shortcut::Conflict(_))
                            && matches!(now, Shortcut::Registered(_))
                        {
                            log.note(&format!("hotkey_conflict_cleared id={which}"));
                        }
                    }
                    shortcuts = s;
                }
                let refreshed = resolve_state(
                    layout_count,
                    &shortcuts,
                    enabled,
                    &runtime_error,
                    &ruleset_failures,
                );
                if refreshed != state {
                    state = refreshed;
                    tray.set_state(&state);
                    tray.notify(&state.summary(), &state.explanation());
                }
                // WM_CONTEXTMENU as well as WM_RBUTTONUP: which one the shell sends depends on
                // the notify-icon protocol version, and accepting both costs nothing.
                if click == WM_RBUTTONUP || click == WM_CONTEXTMENU {
                    command = tray.show_menu(&state, TITLE);
                }
            }
            WM_COMMAND => {
                command = MenuCommand::from_raw(msg.wParam.0 as u32 & 0xFFFF);
            }
            // A second instance declined to start and is pointing the user here.
            window::WM_SHOW_YOURSELF => {
                log.note("second_instance_declined");
                tray.notify(
                    "LexiSwitch is already running",
                    &format!("{} This icon is the one that is running.", state.explanation()),
                );
            }
            _ => {}
        }

        if let Some(command) = command {
            match command {
                MenuCommand::EnableDisable => {
                    enabled = !enabled;
                    // Written back, so the choice survives a restart. The file is the complete
                    // description of behaviour; a setting changed through the interface that
                    // did not appear there would be undone by the next reload.
                    if let Err(e) = persist_setting(&config_path, "general", "enabled",
                                                    if enabled { "true" } else { "false" }) {
                        log.note(&format!("enabled_not_saved: {e}"));
                    }
                    config.general.enabled = enabled;
                    state = resolve_state(
                        layout_count,
                        &shortcuts,
                        enabled,
                        &runtime_error,
                        &ruleset_failures,
                    );
                    tray.set_state(&state);
                    log.note(if enabled { "enabled" } else { "disabled" });
                    tray.notify(&state.summary(), &state.explanation());
                }
                MenuCommand::ChangeCase => {
                    // The same job the shortcut sends, and the only way to change case while
                    // that shortcut is held by another application.
                    if state.converts() {
                        request_case(&work, &config, &state_file, &mut log);
                    } else {
                        tray.notify(&state.summary(), &state.explanation());
                    }
                }
                MenuCommand::Transliterate => {
                    // The same job the shortcut sends, and the only way in while that shortcut
                    // is held elsewhere or has no binding at all.
                    if state.converts() {
                        request_translit(&work, &rulesets, &config, &mut log);
                    } else {
                        tray.notify(&state.summary(), &state.explanation());
                    }
                }
                MenuCommand::OpenTranslitFolder => {
                    open_translit_folder(ruleset_watcher.dir(), &tray)
                }
                MenuCommand::ConvertSelection => {
                    // The same job the hotkey sends, and the only way to convert while the
                    // shortcut is held by another application.
                    if state.converts() {
                        request_conversion(&work, &layouts, &config, &mut log);
                    } else {
                        tray.notify(&state.summary(), &state.explanation());
                    }
                }
                MenuCommand::ReloadConfig => match config_path.as_deref() {
                    Some(path) => match configwatch::load(path) {
                        Loaded::Ok(new) => {
                            config = *new;
                            log.set_enabled(config.logging.enabled);
                            tray.notify("Configuration reloaded", "Settings applied.");
                        }
                        Loaded::Missing => tray.notify(
                            "No configuration file",
                            "Defaults are in effect. Use \"Open configuration file\" to create \
                             one.",
                        ),
                        Loaded::Invalid(why) => tray.notify("Configuration not applied", &why),
                    },
                    None => tray.notify(
                        "No configuration file",
                        "APPDATA is not set, so there is nowhere to look.",
                    ),
                },
                MenuCommand::ChangeShortcut
                | MenuCommand::ChangeCaseShortcut
                | MenuCommand::TransliterateShortcut => {
                    let which = match command {
                        MenuCommand::ChangeShortcut => Which::Convert,
                        MenuCommand::ChangeCaseShortcut => Which::Case,
                        _ => Which::Translit,
                    };
                    change_shortcut(
                        which,
                        &window,
                        &tray,
                        &mut config,
                        &config_path,
                        &mut hotkeys,
                        &mut shortcuts,
                        &mut log,
                    );
                    state = resolve_state(
                        layout_count,
                        &shortcuts,
                        enabled,
                        &runtime_error,
                        &ruleset_failures,
                    );
                    tray.set_state(&state);
                }
                MenuCommand::Diagnostics => {
                    // Everything that decides behaviour, in one balloon. No selected text and
                    // no conversion results: those never leave the transaction they belong to.
                    tray.notify(
                        &state.summary(),
                        &format!(
                            "{}\nVersion {}. Layouts installed: {}. Rulesets: {}. Configuration: {}. Logging: {}.",
                            state.explanation(),
                            VERSION,
                            layout_count,
                            rulesets.iter().map(|r| r.id()).collect::<Vec<_>>().join(", "),
                            config_path
                                .as_deref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "none".into()),
                            if config.logging.enabled { "on" } else { "off" },
                        ),
                    );
                }
                MenuCommand::OpenConfig => open_config(&config_path, &tray),
                MenuCommand::Quit => return Flow::Stop,
            }
        }
        Flow::Continue
    })?;

    // --- shutdown (task 9.8) --------------------------------------------------------
    // Hotkeys are released first so the bindings are free the instant the program stops
    // responding to them, and the worker is joined so nothing is mid-`SendInput` when the
    // process exits. `Worker`, `Hotkeys` and `Tray` all clean up in Drop, so this is belt and
    // braces for the two that carry meaning.
    drop(hotkeys);
    log.note("shutdown");
    drop(work);
    Ok(())
}

/// Ask the worker to convert whatever is selected.
///
/// Reachable from the shortcut and from the tray. The tray path is not a convenience: it is
/// what keeps the product usable while another application holds the shortcut.
fn request_conversion(
    work: &Worker,
    layouts: &[InstalledLayout],
    config: &Config,
    log: &mut EventLog,
) {
    let active = layout_name(layouts, layouts::foreground_langid());
    log.note(&format!(
        "selection_requested active={} app={}",
        active.as_deref().unwrap_or("unknown"),
        process_name(FocusGuard::capture().window())
    ));
    work.send(Job::Selection {
        tables: layouts.iter().map(|l| l.table.clone()).collect(),
        handles: layouts
            .iter()
            .map(|l| (l.table.name().to_string(), l.hkl))
            .collect(),
        active,
        allow_clipboard: config.selection.allow_clipboard_fallback,
        expanded_lines: config.chooser.expanded_lines(),
    });
}

/// Ask the worker to transliterate whatever is selected, under the rulesets as they are now.
///
/// Reachable from the shortcut and from the tray, for the same reason the other two are.
fn request_translit(work: &Worker, rulesets: &Arc<Rulesets>, config: &Config, log: &mut EventLog) {
    log.note(&format!(
        "translit_requested app={} rulesets={}",
        process_name(FocusGuard::capture().window()),
        rulesets.len()
    ));
    work.send(Job::Translit {
        rulesets: Arc::clone(rulesets),
        allow_clipboard: config.selection.allow_clipboard_fallback,
        expanded_lines: config.chooser.expanded_lines(),
    });
}

/// Work out the state, including any ruleset that did not load.
///
/// One place, because the state is recomputed wherever any of its inputs can change, and a
/// recomputation that forgot the rulesets would quietly clear the warning about them.
fn resolve_state(
    layout_count: usize,
    shortcuts: &Shortcuts,
    enabled: bool,
    runtime_error: &Option<String>,
    ruleset_failures: &[RulesetFailure],
) -> AgentState {
    AgentState::resolve(layout_count, shortcuts.clone(), enabled, runtime_error.clone())
        .with_rejected_rulesets(ruleset_failures.iter().map(|f| f.to_string()).collect())
}

/// Rejected ruleset files, as one message: each with its line and its problem.
fn describe_failures(failures: &[RulesetFailure]) -> String {
    let each: Vec<String> = failures.iter().map(|f| f.to_string()).collect();
    format!(
        "{}. Its last good version, if it had one, is still in use, and so is every other \
         ruleset.",
        each.join("; ")
    )
}

/// The bindings that could not be taken, as a sentence — empty when every one registered.
///
/// A shortcut with no binding at all is not here: nothing was attempted, so nothing was refused.
fn unavailable(shortcuts: &Shortcuts) -> String {
    let held: Vec<&str> = [&shortcuts.convert, &shortcuts.case, &shortcuts.translit]
        .into_iter()
        .filter_map(|s| match s {
            Shortcut::Conflict(binding) => Some(binding.as_str()),
            Shortcut::Registered(_) | Shortcut::Unbound { .. } => None,
        })
        .collect();
    match held.len() {
        0 => String::new(),
        1 => format!("{} is held by another application.", held[0]),
        _ => format!("{} are held by other applications.", held.join(" and ")),
    }
}

/// Which of the shortcuts a command is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Convert,
    Case,
    Translit,
}

impl Which {
    /// The configuration key this shortcut is stored under.
    fn key(self) -> &'static str {
        match self {
            Self::Convert => "convert_selection",
            Self::Case => "change_case",
            Self::Translit => "transliterate",
        }
    }

    /// What it does, for a message the user reads.
    fn does(self) -> &'static str {
        match self {
            Self::Convert => "converts the selection",
            Self::Case => "changes case",
            Self::Translit => "transliterates",
        }
    }

    /// This shortcut's state among `shortcuts`.
    fn of(self, shortcuts: &Shortcuts) -> &Shortcut {
        match self {
            Self::Convert => &shortcuts.convert,
            Self::Case => &shortcuts.case,
            Self::Translit => &shortcuts.translit,
        }
    }

    /// This shortcut's binding in `config`, for writing a trial into.
    fn binding(self, config: &mut Config) -> &mut String {
        match self {
            Self::Convert => &mut config.hotkeys.convert_selection,
            Self::Case => &mut config.hotkeys.change_case,
            Self::Translit => &mut config.hotkeys.transliterate,
        }
    }
}

/// Ask the worker to change the case of whatever is selected.
///
/// Reachable from the shortcut and from the tray, for the same reason conversion is: the tray
/// path is what keeps the transaction usable while another application holds the combination.
fn request_case(
    work: &Worker,
    config: &Config,
    state_file: &Option<std::path::PathBuf>,
    log: &mut EventLog,
) {
    log.note(&format!(
        "case_requested app={}",
        process_name(FocusGuard::capture().window())
    ));
    work.send(Job::Case {
        allow_clipboard: config.selection.allow_clipboard_fallback,
        expanded_lines: config.chooser.expanded_lines(),
        state_path: state_file.clone(),
    });
}

/// Record a replacement shortcut, try it, and keep it only if it works.
///
/// Loops until something is registered or the user gives up, because a recorder that closes on
/// a combination it could not take would leave the user exactly where they started with no
/// explanation. A refusal is reported inside the recorder, next to where they will try again.
///
/// Nothing is written to the configuration file until a combination is actually registered. The
/// file describes behaviour, so writing a shortcut that does not work would make it a lie —
/// and on the next start the program would come up in a conflict it had recorded for itself.
#[allow(clippy::too_many_arguments)]
fn change_shortcut(
    which: Which,
    window: &MessageWindow,
    tray: &Tray,
    config: &mut Config,
    config_path: &Option<std::path::PathBuf>,
    hotkeys: &mut Option<Hotkeys>,
    shortcuts: &mut Shortcuts,
    log: &mut EventLog,
) {
    let mut rejected: Option<String> = None;

    loop {
        let current = which.of(shortcuts).clone();
        // Every binding, so the recorder never suggests one this program already holds.
        let taken: Vec<String> = config
            .hotkeys
            .all()
            .iter()
            .map(|(_, b)| b.to_string())
            .collect();
        let recorded = match recorder::show(&current, &taken, rejected.take()) {
            Ok(r) => r,
            Err(e) => {
                tray.notify("Could not open the recorder", &e.to_string());
                return;
            }
        };
        let Recorded::Chosen(binding) = recorded else {
            log.note("shortcut_recording_cancelled");
            return;
        };

        // Release what is held before trying the replacement: a combination cannot be tested
        // while this process is the thing holding it, and re-registering the same one would
        // otherwise look like a conflict with itself. Both go, because the other one is this
        // process too and the user may be moving one onto the other.
        *hotkeys = None;
        let mut trial = config.clone();
        *which.binding(&mut trial) = binding.clone();

        // Refused before it is attempted, because the registration failure would name another
        // application and there is none — the combination is one of this program's own other
        // shortcuts. An unbound shortcut's default is not one of them: it holds nothing.
        let clash = config
            .hotkeys
            .all()
            .iter()
            .filter(|(key, _)| *key != which.key())
            .filter(|(key, _)| !config.unbound().iter().any(|u| u.key == *key))
            .any(|(_, other)| other.trim().eq_ignore_ascii_case(binding.trim()));
        if clash {
            let (h, s) = register_hotkeys(window, config, log);
            *hotkeys = h;
            *shortcuts = s;
            rejected = Some(format!(
                "{binding} is already another LexiSwitch shortcut. One combination cannot \
                 open two windows."
            ));
            continue;
        }

        let (registered, attempted) = register_hotkeys(window, &trial, log);
        let took = matches!(which.of(&attempted), Shortcut::Registered(_));

        if took {
            *hotkeys = registered;
            *config = trial;
            *shortcuts = attempted;
            log.note("shortcut_changed");

            let written = persist_setting(
                config_path,
                "hotkeys",
                which.key(),
                &format!("\"{}\"", binding.replace('"', "\\\"")),
            );
            tray.notify(
                "Shortcut changed",
                &match written {
                    Ok(()) => format!("{binding} now {}.", which.does()),
                    Err(e) => format!(
                        "{binding} now {}, but it could not be saved: {e}. It will revert \
                         when LexiSwitch restarts.",
                        which.does()
                    ),
                },
            );
            return;
        }

        // Put back whatever worked before. Silence would be the worst outcome of an attempt
        // to fix silence.
        let (h, s) = register_hotkeys(window, config, log);
        *hotkeys = h;
        *shortcuts = s;
        log.note("shortcut_rejected");
        rejected = Some(format!("{binding} is not available."));
    }
}

/// Write one setting into the configuration file, leaving the rest of it alone.
fn persist_setting(
    config_path: &Option<std::path::PathBuf>,
    table: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let Some(path) = config_path else {
        return Err("APPDATA is not set, so there is nowhere to save it".into());
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let updated = lexiswitch_core::config::rewrite_setting(&existing, table, key, value);
    std::fs::write(path, updated).map_err(|e| e.to_string())
}

/// Claim every binding, keeping whatever the system will give.
///
/// Returns what registered and the state of each binding. Never an error: one combination held
/// by another application is a condition to report, not a reason to abandon the other — and a
/// binding that cannot even be parsed is the same kind of problem from the user's side, so it
/// lands in the same state rather than in a separate failure path.
fn register_hotkeys(
    window: &MessageWindow,
    config: &Config,
    log: &mut EventLog,
) -> (Option<Hotkeys>, Shortcuts) {
    let wanted = [
        (HotkeyId::ConvertSelection, &config.hotkeys.convert_selection),
        (HotkeyId::ChangeCase, &config.hotkeys.change_case),
        (HotkeyId::Transliterate, &config.hotkeys.transliterate),
    ];
    // A shortcut whose default gave way to a binding the user wrote is not attempted at all:
    // registering it would collide with this program's own other shortcut.
    let unbound = |id: HotkeyId| config.unbound().iter().find(|u| u.key == id.config_key());

    let mut bindings = Vec::new();
    let mut unparsed = Vec::new();
    for (id, text) in wanted.into_iter().filter(|(id, _)| unbound(*id).is_none()) {
        match parse_binding(text) {
            Ok(b) => bindings.push((id, b)),
            Err(e) => {
                log.note(&format!("binding_unparsed id={id:?}"));
                unparsed.push((id, e));
            }
        }
    }

    let outcome = Hotkeys::register(window.hwnd(), &bindings);
    for failure in &outcome.failed {
        log.note(&format!("hotkey_conflict: {failure}"));
    }

    let status = |id: HotkeyId, text: &str| {
        if let Some(u) = unbound(id) {
            Shortcut::Unbound {
                binding: u.binding.clone(),
                taken_by: u.taken_by,
            }
        } else if unparsed.iter().any(|(u, _)| *u == id) || !outcome.hotkeys.holds(id) {
            Shortcut::Conflict(text.to_string())
        } else {
            Shortcut::Registered(text.to_string())
        }
    };
    let shortcuts = Shortcuts {
        convert: status(
            HotkeyId::ConvertSelection,
            &config.hotkeys.convert_selection,
        ),
        case: status(HotkeyId::ChangeCase, &config.hotkeys.change_case),
        translit: status(HotkeyId::Transliterate, &config.hotkeys.transliterate),
    };

    (Some(outcome.hotkeys), shortcuts)
}

fn open_config(path: &Option<std::path::PathBuf>, tray: &Tray) {
    let Some(path) = path else {
        tray.notify("No configuration file", "APPDATA is not set.");
        return;
    };
    if !path.exists() {
        // Create the directory and seed the file, so "open" always opens something.
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(path, include_str!("../../config.example.toml")) {
            tray.notify("Could not create configuration", &e.to_string());
            return;
        }
    }
    // `explorer` opens the file with whatever is registered for .toml, without this process
    // needing to know or care what that is.
    if let Err(e) = std::process::Command::new("explorer").arg(path).spawn() {
        tray.notify("Could not open configuration", &e.to_string());
    }
}

/// Open the transliteration folder in Explorer, creating it empty if it does not exist.
///
/// Creates the folder and nothing in it. The rulesets there are the user's own files; the program
/// reads them and never writes one (ADR-0017). The built-ins and the example shipped beside the
/// program show what to put there.
fn open_translit_folder(dir: Option<&std::path::Path>, tray: &Tray) {
    let Some(dir) = dir else {
        tray.notify("No transliteration folder", "APPDATA is not set.");
        return;
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        tray.notify("Could not create the transliteration folder", &e.to_string());
        return;
    }
    if let Err(e) = std::process::Command::new("explorer").arg(dir).spawn() {
        tray.notify("Could not open the transliteration folder", &e.to_string());
    }
}

/// The table name for a language id, for use as the weak ranking hint.
///
/// All that survives of `pick_pair`, which chose a conversion's *source* from the foreground
/// window's active input language. That is a fact about the present and not evidence about
/// text that may have been typed at any time, which is why selecting mistyped text after
/// switching layouts silently did nothing at all.
fn layout_name(layouts: &[InstalledLayout], langid: u16) -> Option<String> {
    layouts
        .iter()
        .find(|l| l.langid == langid)
        .map(|l| l.table.name().to_string())
}
