//! The binary: wiring, configuration, and the event loop. Logic lives in `lexiswitch-core`,
//! Win32 in `lexiswitch-win`; this crate holds neither.
//!
//! Startup and the two-thread split are task 9.1 and 9.2. The message-loop thread owns the
//! keystroke buffer outright and never blocks; the worker thread receives snapshots over a
//! channel and does the automation, clipboard, and injection work that can stall.

// This crate wires two crates together; it has no reason to dereference a raw pointer.
#![forbid(unsafe_code)]
// No console window in the shipped build — LexiSwitch is a tray agent. Left to the release
// profile only, so a debug run still has somewhere to print.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Task 9.1: load config, build layout tables, register hotkeys, create the message
    // window, start the worker thread.
    eprintln!("lexiswitch: not implemented yet");
}
