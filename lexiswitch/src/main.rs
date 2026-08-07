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

use lexiswitch_win::focus::process_name;
use lexiswitch_win::hotkey::{parse_binding, Binding, HotkeyId, Hotkeys};
use lexiswitch_win::inject::FocusGuard;
use lexiswitch_win::layouts::{self, InstalledLayout};
use lexiswitch_win::tray::{tray_event, MenuCommand, State, Tray, WM_TRAY};
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
            Loaded::Ok(config) => config,
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

    // Reported, not silently dropped. A removed setting and a misspelled one both land here,
    // and naming them is the only way the user can tell which they are looking at. Held until
    // the tray exists, since that is where the user will see it.
    let ignored_at_startup = config.ignored_keys();

    // --- layouts, window, hotkeys ---------------------------------------------------
    let layouts = layouts::build_all()?;
    log.note(&format!("layouts_built count={}", layouts.len()));

    let window = MessageWindow::new()?;
    let mut hotkeys = Some(register_hotkeys(&window, &config)?);

    let tray = Tray::new(window.hwnd(), "LexiSwitch")?;
    tray.notify(
        "LexiSwitch",
        &format!(
            "Ready. Select text and press {} to convert it.",
            config.hotkeys.convert_selection
        ),
    );
    if !ignored_at_startup.is_empty() {
        tray.notify(
            "Some settings were ignored",
            &format!(
                "{} is not a setting this version has. It was ignored and everything else                  was applied. Check the spelling if you did not expect this.",
                ignored_at_startup.join(", ")
            ),
        );
    }

    // Whether the hotkey converts. Replaces the old paused state, which referred to keystroke
    // recording that no longer happens.
    let mut state = State::Enabled;

    // --- the worker ----------------------------------------------------------------
    let (work, outcomes) = Worker::spawn(window.waker());
    let mut watcher = config_path.clone().map(ConfigWatcher::new);

    window.run(|msg| {
        // Anything the worker finished gets reported here, on the thread that owns the tray.
        while let Ok(outcome) = outcomes.try_recv() {
            match outcome {
                Outcome::Quiet => log.note("outcome_quiet"),
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

        match msg.message {
            WM_HOTKEY => {
                if state == State::Disabled {
                    return Flow::Continue;
                }
                if let Some(HotkeyId::ConvertSelection) = HotkeyId::from_raw(msg.wParam.0 as i32) {
                    // Every installed layout goes across, not a chosen pair. Which conversions
                    // are plausible is decided from the selected characters; the active layout
                    // travels only as a weak tiebreak on the target, because it is a fact about
                    // the present and says nothing about text typed at any earlier time
                    // (ADR-0012).
                    let active = layout_name(&layouts, layouts::foreground_langid());
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
                    });
                }
            }
            // Logged unconditionally, before any guard. Two rounds of fixing this menu have
            // assumed the message arrives and only the handling was wrong; nothing has yet
            // shown that it arrives at all. `lparam` carries the mouse message under the
            // legacy tray protocol this icon uses.
            WM_TRAY => {
                let click = tray_event(msg.lParam.0);
                log.note(&format!("tray_message event=0x{click:04X}"));
                // WM_CONTEXTMENU as well as WM_RBUTTONUP: which one the shell sends depends on
                // the notify-icon protocol version, and accepting both costs nothing.
                if click != WM_RBUTTONUP && click != WM_CONTEXTMENU {
                    return Flow::Continue;
                }
                if let Some(command) = tray.show_menu(state) {
                    match command {
                        MenuCommand::EnableDisable => {
                            state = match state {
                                State::Enabled => State::Disabled,
                                State::Disabled => State::Enabled,
                            };
                            tray.set_state(state);
                            match state {
                                State::Enabled => {
                                    log.note("enabled");
                                    tray.notify(
                                        "Enabled",
                                        "The hotkey converts the selection again.",
                                    );
                                }
                                State::Disabled => {
                                    log.note("disabled");
                                    tray.notify(
                                        "Disabled",
                                        "The hotkey no longer converts. Nothing is being \
                                         recorded either way.",
                                    );
                                }
                            }
                        }
                        MenuCommand::ReloadConfig => match config_path.as_deref() {
                            Some(path) => match configwatch::load(path) {
                                Loaded::Ok(new) => {
                                    config = new;
                                    log.set_enabled(config.logging.enabled);
                                    tray.notify("Configuration reloaded", "Settings applied.");
                                }
                                Loaded::Missing => tray.notify(
                                    "No configuration file",
                                    "Defaults are in effect. Use \"Open configuration file\" to \
                                     create one.",
                                ),
                                Loaded::Invalid(why) => {
                                    tray.notify("Configuration not applied", &why)
                                }
                            },
                            None => tray.notify(
                                "No configuration file",
                                "APPDATA is not set, so there is nowhere to look.",
                            ),
                        },
                        MenuCommand::OpenConfig => open_config(&config_path, &tray),
                        MenuCommand::Quit => return Flow::Stop,
                    }
                }
            }
            WM_COMMAND => {
                if let Some(MenuCommand::Quit) = MenuCommand::from_raw(msg.wParam.0 as u32 & 0xFFFF)
                {
                    return Flow::Stop;
                }
            }
            _ => {}
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
