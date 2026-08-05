//! The binary: wiring, configuration, and the event loop. Logic lives in `lexiswitch-core`,
//! Win32 in `lexiswitch-win`; this crate holds neither.
//!
//! Two threads, split by what is allowed to be slow. This one runs the window, Raw Input,
//! hotkeys and the tray, and never blocks — it owns the keystroke buffer outright, so the
//! most sensitive data in the program is never concurrent and needs no lock. Anything that
//! can stall (UI Automation, the clipboard, input synthesis) happens on the worker, reached
//! by sending it a snapshot.

// This crate wires two crates together; it has no reason to dereference a raw pointer.
#![forbid(unsafe_code)]
// No console window in the shipped build — LexiSwitch is a tray agent. Left to the release
// profile only, so a debug run still has somewhere to print.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use lexiswitch_core::buffer::{KeyBuffer, WipeReason};
use lexiswitch_core::config::Config;
use lexiswitch_core::cycle::{render, MultiTap};
use lexiswitch_core::layout::LayoutTable;
use lexiswitch_win::focus::PasswordGate;
use lexiswitch_win::hotkey::{parse_binding, Binding, HotkeyId, Hotkeys};
use lexiswitch_win::inject::FocusGuard;
use lexiswitch_win::layouts::{self, InstalledLayout};
use lexiswitch_win::pagelock::LockedBuffer;
use lexiswitch_win::rawinput::{self, KeyFilter, RawKeyboard};
use lexiswitch_win::tray::{MenuCommand, Tray, WM_TRAY};
use lexiswitch_win::window::{Flow, MessageWindow};
use windows::Win32::UI::WindowsAndMessaging::{WM_COMMAND, WM_HOTKEY, WM_INPUT, WM_RBUTTONUP};

mod configwatch;
mod worker;

use configwatch::{ConfigWatcher, EventLog, Loaded};
use worker::{Job, Outcome, Worker};

/// Temporary diagnostic scaffolding from the section 6 investigation.
///
/// **This prints keystroke content, which the shipped logging must never do** (ADR-0005,
/// task 9.7 — see [`EventLog`], which takes lengths and cannot be handed text). Debug-build
/// only, off unless `LEXISWITCH_DIAG` is set, stderr only. Remove once section 8's
/// milestones pass.
macro_rules! diag {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if std::env::var_os("LEXISWITCH_DIAG").is_some() {
                eprintln!($($arg)*);
            }
        }
    };
}

fn main() {
    if let Err(e) = run() {
        // Startup failures happen before the tray exists, so they have nowhere else to go.
        eprintln!("lexiswitch: {e}");
        std::process::exit(1);
    }
}

/// A correction in progress, retained across the multi-tap window.
///
/// The source text is kept so every press re-renders from it rather than from the previous
/// result — that is what makes a press idempotent for its count and stops repeated taps
/// drifting (ADR-0009). `on_screen` tracks what the document currently shows, because that
/// is what the next press has to backspace over.
struct Correction {
    source: String,
    on_screen: String,
    from_langid: u16,
}

/// What was typed, and under which layout.
///
/// The layout is captured **as each keystroke arrives**, not when the hotkey fires. The two
/// differ whenever the active layout changes in between, and LexiSwitch changes it itself
/// when configured to, so the divergence is guaranteed rather than hypothetical.
#[derive(Default)]
struct TypingContext {
    langid: Option<u16>,
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

    // --- layouts, window, input ----------------------------------------------------
    let layouts = layouts::build_all()?;
    log.note(&format!("layouts_built count={}", layouts.len()));
    eprintln!(
        "layouts: {}",
        layouts
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let window = MessageWindow::new()?;
    let _raw = RawKeyboard::register(window.hwnd())?;
    let mut hotkeys = Some(register_hotkeys(&window, &config)?);

    let tray = Tray::new(window.hwnd(), "LexiSwitch")?;
    tray.notify(
        "LexiSwitch",
        &format!(
            "Ready. {} converts the last word you typed.",
            config.hotkeys.convert_last_word
        ),
    );

    // Page-locked to keep recent keystrokes out of the pagefile (ADR-0005). Best-effort: the
    // working-set limit can refuse, which must not stop the program — but it does weaken a
    // stated guarantee, so it is reported rather than swallowed.
    let (mut buffer, lock_error) = LockedBuffer::new(KeyBuffer::with_idle_timeout(
        config.buffer.idle_timeout_secs * 1_000,
    ));
    if let Some(e) = lock_error {
        eprintln!("lexiswitch: buffer pages not locked ({e}); keystrokes may reach the pagefile");
    }

    let mut gate = PasswordGate::new();
    if !gate.has_automation() {
        eprintln!(
            "lexiswitch: UI Automation unavailable; password-field detection falls back to \
             the ES_PASSWORD window style, which is weak outside native Win32 controls"
        );
    }

    // --- the worker ----------------------------------------------------------------
    let (work, outcomes) = Worker::spawn();

    let mut filter = KeyFilter::new();
    let mut taps = MultiTap::new(config.hotkeys.multi_tap_window_ms);
    let mut correction: Option<Correction> = None;
    let mut typing = TypingContext::default();
    let mut watcher = config_path.clone().map(ConfigWatcher::new);

    window.run(|msg| {
        // Anything the worker finished gets reported here, on the thread that owns the tray.
        while let Ok(outcome) = outcomes.try_recv() {
            match outcome {
                Outcome::Quiet => {}
                Outcome::Note { title, body } | Outcome::Failed { title, body } => {
                    tray.notify(&title, &body)
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
                        tray.notify("Configuration reloaded", "The new settings are in effect.");
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
            WM_INPUT => {
                if let Some(event) = rawinput::decode(msg.lParam.0) {
                    let accepted = filter.accept(event);
                    if let Some(key) = accepted {
                        // Task 7.5 — three independent reasons not to record, checked before
                        // anything is retained rather than filtered afterwards.
                        let focus = gate.current();
                        let blocked = if focus.is_password {
                            Some("password field")
                        } else if config.exclusions.is_excluded(&focus.process) {
                            Some("excluded application")
                        } else if buffer.is_paused() {
                            Some("paused")
                        } else {
                            None
                        };
                        if let Some(reason) = blocked {
                            diag!("     NOT RECORDED ({reason}) process={:?}", focus.process);
                            log.note("keystroke_ignored");
                            // Anything already held was typed somewhere else; moving into a
                            // password field must not leave it recoverable.
                            buffer.wipe();
                            return Flow::Continue;
                        }

                        buffer.record(key.scancode, key.shift, msg.time);
                        typing.langid = Some(layouts::foreground_langid());
                        correction = None;
                        taps.reset();
                    }
                }
            }
            WM_HOTKEY => {
                // Timestamps come from the message, never a clock read in this handler:
                // scheduling delay here would corrupt the multi-tap count.
                let press = taps.press(msg.time);
                match HotkeyId::from_raw(msg.wParam.0 as i32) {
                    Some(HotkeyId::ConvertLastWord) => convert_last_word(
                        &config,
                        &tray,
                        &log,
                        &work,
                        &layouts,
                        &mut buffer,
                        &mut correction,
                        &typing,
                        msg.time,
                        press,
                    ),
                    Some(HotkeyId::ConvertSelection) => {
                        if let Some((from, to)) = pick_pair(&layouts, press) {
                            log.note("selection_requested");
                            work.send(Job::Selection {
                                from,
                                to,
                                allow_clipboard: config.selection.allow_clipboard_fallback,
                            });
                        }
                    }
                    Some(HotkeyId::ConvertClipboard) => {
                        if let Some((from, to)) = pick_pair(&layouts, press) {
                            log.note("clipboard_requested");
                            work.send(Job::ClipboardOnly { from, to });
                        }
                    }
                    Some(HotkeyId::PauseToggle) => {
                        {
                            let want = !buffer.is_paused();
                            set_paused(&mut buffer, &tray, &log, want)
                        }
                    }
                    None => {}
                }
            }
            WM_TRAY if msg.lParam.0 as u32 == WM_RBUTTONUP => {
                if let Some(command) = tray.show_menu(buffer.is_paused()) {
                    match command {
                        MenuCommand::PauseResume => {
                            {
                            let want = !buffer.is_paused();
                            set_paused(&mut buffer, &tray, &log, want)
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
    // Order matters. Hotkeys are released first so the bindings are free the instant the
    // program stops responding to them; the buffer is wiped before its memory is returned;
    // the worker is joined so nothing is mid-SendInput when the process exits. `Worker`,
    // `Hotkeys`, `Tray`, `RawKeyboard` and `LockedBuffer` all clean up in Drop, so this is
    // belt and braces for the two that carry meaning.
    drop(hotkeys);
    buffer.wipe();
    log.note("shutdown");
    drop(work);
    Ok(())
}

fn register_hotkeys(window: &MessageWindow, config: &Config) -> Result<Hotkeys, Box<dyn Error>> {
    let bindings: Vec<(HotkeyId, Binding)> = [
        (HotkeyId::ConvertLastWord, &config.hotkeys.convert_last_word),
        (HotkeyId::ConvertSelection, &config.hotkeys.convert_selection),
        (HotkeyId::ConvertClipboard, &config.hotkeys.convert_clipboard),
        (HotkeyId::PauseToggle, &config.hotkeys.pause_toggle),
    ]
    .into_iter()
    .map(|(id, text)| parse_binding(text).map(|b| (id, b)))
    .collect::<Result<Vec<_>, _>>()?;

    Ok(Hotkeys::register(window.hwnd(), &bindings)?)
}

fn set_paused(buffer: &mut LockedBuffer, tray: &Tray, log: &EventLog, paused: bool) {
    buffer.set_paused(paused);
    tray.set_paused(paused);
    log.note(if paused { "paused" } else { "resumed" });
    tray.notify(
        if paused { "Paused" } else { "Resumed" },
        if paused {
            "Keystrokes are no longer recorded, and what was held has been wiped."
        } else {
            "Recording keystrokes again."
        },
    );
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

/// Source and target layout tables for the current press, cloned so they can cross to the
/// worker without borrowing anything the message loop owns.
fn pick_pair(layouts: &[InstalledLayout], press: usize) -> Option<(LayoutTable, LayoutTable)> {
    let langid = layouts::foreground_langid();
    let from = layouts
        .iter()
        .find(|l| l.langid == langid)
        .or_else(|| layouts.first())?;
    let candidates: Vec<&InstalledLayout> =
        layouts.iter().filter(|l| l.langid != from.langid).collect();
    if candidates.is_empty() {
        return None;
    }
    let to = candidates[(press - 1) % candidates.len()];
    Some((from.table.clone(), to.table.clone()))
}

/// Convert the last typed word in place, cycling on repeated presses.
#[allow(clippy::too_many_arguments)]
fn convert_last_word(
    config: &Config,
    tray: &Tray,
    log: &EventLog,
    work: &Worker,
    layouts: &[InstalledLayout],
    buffer: &mut LockedBuffer,
    correction: &mut Option<Correction>,
    typing: &TypingContext,
    now: u32,
    press: usize,
) {
    // Captured on this thread, checked on the worker immediately before injecting.
    let guard = FocusGuard::capture();

    if press == 1 {
        let Some(langid) = typing.langid else {
            tray.notify(
                "Nothing to correct",
                "No recent typing was recorded, or it has gone stale.",
            );
            return;
        };
        let Some(active) = layout_for(layouts, langid) else {
            return;
        };
        match buffer.last_word(&active.table, now) {
            Some(word) => {
                log.event("word_extracted", word.chars().count());
                *correction = Some(Correction {
                    on_screen: word.clone(),
                    source: word,
                    from_langid: active.langid,
                });
            }
            None => {
                *correction = None;
                tray.notify(
                    "Nothing to correct",
                    "No recent typing was recorded, or it has gone stale.",
                );
                return;
            }
        }
    }

    let Some(state) = correction.as_mut() else {
        return;
    };
    let Some(from) = layout_for(layouts, state.from_langid) else {
        return;
    };

    let candidates: Vec<&InstalledLayout> = layouts
        .iter()
        .filter(|l| l.langid != state.from_langid)
        .collect();
    let tables: Vec<&LayoutTable> = candidates.iter().map(|l| &l.table).collect();

    let rendered = render(&state.source, &from.table, &tables, press);
    diag!(
        "  press={press} source={:?} on_screen={:?} rendered={:?}",
        state.source,
        state.on_screen,
        rendered
    );
    if rendered == state.on_screen {
        // Rewriting a word with itself would disturb the cursor for no reason.
        return;
    }

    let target = if candidates.is_empty() {
        from
    } else {
        let index = (press - 1) % (candidates.len() + 1);
        candidates.get(index).copied().unwrap_or(from)
    };

    let backspaces = state.on_screen.chars().count();
    log.event("correction_sent", rendered.chars().count());
    // The slow part happens on the worker; this thread returns to pumping messages.
    work.send(Job::Replace {
        guard,
        backspaces,
        text: rendered.clone(),
        activate: config
            .behavior
            .switch_layout_after_correction
            .then_some(target.hkl),
    });
    state.on_screen = rendered;

    // A conversion invalidates the buffer (ADR-0005). The retained source survives only for
    // the multi-tap window, in `correction`.
    buffer.invalidate(WipeReason::Converted);
}

/// The installed layout for a language id, falling back to the first installed one.
fn layout_for(layouts: &[InstalledLayout], langid: u16) -> Option<&InstalledLayout> {
    layouts
        .iter()
        .find(|l| l.langid == langid)
        .or_else(|| layouts.first())
}
