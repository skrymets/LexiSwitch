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

use lexiswitch_core::config::Config;
use lexiswitch_core::state::{AgentState, Shortcut};

use lexiswitch_win::focus::process_name;
use lexiswitch_win::hotkey::{parse_binding, Binding, HotkeyId, Hotkeys};
use lexiswitch_win::recorder::{self, Recorded};
use lexiswitch_win::inject::FocusGuard;
use lexiswitch_win::layouts::{self, InstalledLayout};
use lexiswitch_win::tray::{tray_event, MenuCommand, Tray, WM_TRAY};
use lexiswitch_win::window::{Flow, MessageWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_COMMAND, WM_CONTEXTMENU, WM_HOTKEY, WM_RBUTTONUP,
};

mod configwatch;
mod worker;

use configwatch::{ConfigWatcher, EventLog, Loaded};
use worker::{Job, Outcome, Worker};

fn main() {
    if let Err(e) = run() {
        // Startup failures happen before the tray exists, so they have nowhere else to go.
        eprintln!("lexiswitch: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    // --- configuration -------------------------------------------------------------
    let config_path = configwatch::default_path();
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

    // A shortcut another application holds is a state, not an exit.
    let mut hotkeys = None;
    let mut shortcut = match register_hotkeys(&window, &config) {
        Ok(registered) => {
            hotkeys = Some(registered);
            Shortcut::Registered(config.hotkeys.convert_selection.clone())
        }
        Err(e) => {
            log.note(&format!("hotkey_conflict: {e}"));
            Shortcut::Conflict(config.hotkeys.convert_selection.clone())
        }
    };

    let mut enabled = config.general.enabled;
    let mut state = AgentState::resolve(
        layout_count,
        shortcut.clone(),
        enabled,
        runtime_error.clone(),
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
                            match register_hotkeys(&window, &config) {
                                Ok(h) => hotkeys = Some(h),
                                Err(e) => tray.notify("Hotkeys not registered", &e.to_string()),
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
            }
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
                if matches!(shortcut, Shortcut::Conflict(_)) {
                    if let Ok(registered) = register_hotkeys(&window, &config) {
                        log.note("hotkey_conflict_cleared");
                        hotkeys = Some(registered);
                        shortcut = Shortcut::Registered(config.hotkeys.convert_selection.clone());
                    }
                }
                let refreshed = AgentState::resolve(
                    layout_count,
                    shortcut.clone(),
                    enabled,
                    runtime_error.clone(),
                );
                if refreshed != state {
                    state = refreshed;
                    tray.set_state(&state);
                    tray.notify(&state.summary(), &state.explanation());
                }
                // WM_CONTEXTMENU as well as WM_RBUTTONUP: which one the shell sends depends on
                // the notify-icon protocol version, and accepting both costs nothing.
                if click == WM_RBUTTONUP || click == WM_CONTEXTMENU {
                    command = tray.show_menu(&state);
                }
            }
            WM_COMMAND => {
                command = MenuCommand::from_raw(msg.wParam.0 as u32 & 0xFFFF);
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
                    state = AgentState::resolve(
                        layout_count,
                        shortcut.clone(),
                        enabled,
                        runtime_error.clone(),
                    );
                    tray.set_state(&state);
                    log.note(if enabled { "enabled" } else { "disabled" });
                    tray.notify(&state.summary(), &state.explanation());
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
                MenuCommand::ChangeShortcut => {
                    change_shortcut(
                        &window,
                        &tray,
                        &mut config,
                        &config_path,
                        &mut hotkeys,
                        &mut shortcut,
                        &mut log,
                    );
                    state = AgentState::resolve(
                        layout_count,
                        shortcut.clone(),
                        enabled,
                        runtime_error.clone(),
                    );
                    tray.set_state(&state);
                }
                MenuCommand::Diagnostics => {
                    // Everything that decides behaviour, in one balloon. No selected text and
                    // no conversion results: those never leave the transaction they belong to.
                    tray.notify(
                        &state.summary(),
                        &format!(
                            "{}\nLayouts installed: {}. Configuration: {}. Logging: {}.",
                            state.explanation(),
                            layout_count,
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

/// Record a replacement shortcut, try it, and keep it only if it works.
///
/// Loops until something is registered or the user gives up, because a recorder that closes on
/// a combination it could not take would leave the user exactly where they started with no
/// explanation. A refusal is reported inside the recorder, next to where they will try again.
///
/// Nothing is written to the configuration file until a combination is actually registered. The
/// file describes behaviour, so writing a shortcut that does not work would make it a lie —
/// and on the next start the program would come up in a conflict it had recorded for itself.
fn change_shortcut(
    window: &MessageWindow,
    tray: &Tray,
    config: &mut Config,
    config_path: &Option<std::path::PathBuf>,
    hotkeys: &mut Option<Hotkeys>,
    shortcut: &mut Shortcut,
    log: &mut EventLog,
) {
    let mut rejected: Option<String> = None;

    loop {
        let recorded = match recorder::show(shortcut, rejected.take()) {
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
        // otherwise look like a conflict with itself.
        *hotkeys = None;
        let mut trial = config.clone();
        trial.hotkeys.convert_selection = binding.clone();

        match register_hotkeys(window, &trial) {
            Ok(registered) => {
                *hotkeys = Some(registered);
                *config = trial;
                *shortcut = Shortcut::Registered(binding.clone());
                log.note("shortcut_changed");

                let written = persist_shortcut(config_path, &binding);
                tray.notify(
                    "Shortcut changed",
                    &match written {
                        Ok(()) => format!("{binding} now converts the selection."),
                        Err(e) => format!(
                            "{binding} now converts the selection, but it could not be saved: \
                             {e}. It will revert when LexiSwitch restarts."
                        ),
                    },
                );
                return;
            }
            Err(e) => {
                // Put back whatever worked before. Silence would be the worst outcome of an
                // attempt to fix silence.
                if matches!(shortcut, Shortcut::Registered(_)) {
                    *hotkeys = register_hotkeys(window, config).ok();
                }
                log.note("shortcut_rejected");
                rejected = Some(format!("{binding} is not available: {e}"));
            }
        }
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

/// Write the shortcut into the configuration file, leaving the rest of it alone.
fn persist_shortcut(
    config_path: &Option<std::path::PathBuf>,
    binding: &str,
) -> Result<(), String> {
    let Some(path) = config_path else {
        return Err("APPDATA is not set, so there is nowhere to save it".into());
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // An unreadable file is treated as an absent one rather than as a reason to refuse: the
    // rewrite creates what it needs, and failing here would leave a working shortcut unsaved
    // for no benefit.
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let updated = lexiswitch_core::config::rewrite_hotkey(&existing, binding);
    std::fs::write(path, updated).map_err(|e| e.to_string())
}

fn register_hotkeys(window: &MessageWindow, config: &Config) -> Result<Hotkeys, Box<dyn Error>> {
    let bindings: Vec<(HotkeyId, Binding)> =
        [(HotkeyId::ConvertSelection, &config.hotkeys.convert_selection)]
            .into_iter()
            .map(|(id, text)| parse_binding(text).map(|b| (id, b)))
            .collect::<Result<Vec<_>, _>>()?;

    Ok(Hotkeys::register(window.hwnd(), &bindings)?)
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
