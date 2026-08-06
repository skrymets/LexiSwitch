//! The worker thread: everything that is allowed to be slow.
//!
//! The message-loop thread runs the window, Raw Input, hotkeys and the tray, and must never
//! block — a stalled loop means a dead tray icon and, worse, a program that appears to have
//! frozen the keyboard. UI Automation and clipboard calls can take seconds, so they live
//! here instead, reached over a channel.
//!
//! The keystroke buffer never crosses this boundary. The message-loop thread owns it
//! outright and sends a *snapshot* of the extracted text, which is why the buffer needs no
//! lock and the most sensitive data in the program is never concurrent
//! ([ADR-0005](../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md)).
//!
//! Neither does any COM interface. UI Automation is apartment-bound, so this thread
//! initializes its own rather than accepting one built elsewhere.

use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use lexiswitch_core::layout::LayoutTable;
use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::Uia;
use lexiswitch_win::inject::{self, FocusGuard};
use lexiswitch_win::langswitch;
use lexiswitch_win::selection;

/// Work handed to the worker. Everything here is owned data — nothing borrows from the
/// message-loop thread, and no handle that is thread-affine crosses.
pub enum Job {
    /// Replace text already on screen: delete `backspaces` characters, type `text`.
    Replace {
        guard: FocusGuard,
        backspaces: usize,
        text: String,
        /// Layout to activate afterward, when configured.
        activate: Option<u32>,
    },
    /// Convert whatever is selected, automation first.
    Selection {
        from: LayoutTable,
        to: LayoutTable,
        allow_clipboard: bool,
    },
    /// Convert the clipboard in place, pasting nothing.
    ClipboardOnly { from: LayoutTable, to: LayoutTable },
    /// Finish and let the thread exit.
    Stop,
}

/// What happened, for the message-loop thread to show in the tray.
pub enum Outcome {
    /// Nothing worth telling the user about.
    Quiet,
    /// Something worked, and the user should know a side effect occurred.
    Note { title: String, body: String },
    /// Something did not happen. The body says what.
    Failed { title: String, body: String },
}

/// Handle to a running worker.
pub struct Worker {
    tx: Sender<Job>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// Spawn the worker and return a handle plus the channel its outcomes arrive on.
    pub fn spawn() -> (Self, Receiver<Outcome>) {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let (out_tx, out_rx) = std::sync::mpsc::channel::<Outcome>();

        let handle = std::thread::Builder::new()
            .name("lexiswitch-worker".into())
            .spawn(move || run(rx, out_tx))
            .expect("spawning the worker thread");

        (
            Self {
                tx,
                handle: Some(handle),
            },
            out_rx,
        )
    }

    /// Hand over a job. Returns false if the worker has gone away, which is only possible
    /// during shutdown.
    pub fn send(&self, job: Job) -> bool {
        self.tx.send(job).is_ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Ask the worker to finish, then wait for it — task 9.8 requires no process
        // remaining, and a detached thread mid-SendInput is exactly what that forbids.
        let _ = self.tx.send(Job::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run(rx: Receiver<Job>, out: Sender<Outcome>) {
    // This thread's own automation object. COM apartments are per-thread and an interface
    // created elsewhere must not be used here, so it is built rather than borrowed.
    let uia = Uia::new();

    while let Ok(job) = rx.recv() {
        let outcome = match job {
            Job::Stop => break,
            Job::Replace {
                guard,
                backspaces,
                text,
                activate,
            } => match inject::replace(&guard, backspaces, &text) {
                Ok(()) => {
                    if let Some(hkl) = activate {
                        // Advisory: an application may decline, and there is no reliable way
                        // to learn that it did.
                        let _ = langswitch::activate_for_foreground(hkl);
                    }
                    Outcome::Quiet
                }
                Err(e) => Outcome::Failed {
                    title: "Correction not applied".into(),
                    body: e.to_string(),
                },
            },
            Job::Selection {
                from,
                to,
                allow_clipboard,
            } => match selection::convert_selection(
                uia.as_ref().and_then(|u| u.automation()),
                // No owner window: the worker owns none, and the clipboard does not need
                // one for reading and writing.
                windows::Win32::Foundation::HWND(std::ptr::null_mut()),
                &from,
                &to,
                allow_clipboard,
                5,
            ) {
                // Only when something was actually destroyed. Every conversion goes through
                // the clipboard now (ADR-0010), so announcing that it did would be a
                // notification on every single use and would train the user to dismiss it.
                Ok(done) if done.lost_nontext => Outcome::Note {
                    title: "Selection converted".into(),
                    body: "The clipboard held an image or formatted content, which the \
                           conversion destroyed. Text on it was restored."
                        .into(),
                },
                Ok(_) => Outcome::Quiet,
                Err(e) => Outcome::Failed {
                    title: "Selection not converted".into(),
                    body: e.to_string(),
                },
            },
            Job::ClipboardOnly { from, to } => convert_clipboard(&from, &to),
        };

        if out.send(outcome).is_err() {
            // The message loop has gone; nothing left to report to.
            break;
        }
    }
}

fn convert_clipboard(from: &LayoutTable, to: &LayoutTable) -> Outcome {
    let owner = windows::Win32::Foundation::HWND(std::ptr::null_mut());
    let clip = match Clipboard::open(owner, 5) {
        Ok(c) => c,
        Err(e) => {
            return Outcome::Failed {
                title: "Clipboard not converted".into(),
                body: e.to_string(),
            }
        }
    };
    let Some(text) = clip.text() else {
        return Outcome::Failed {
            title: "Clipboard not converted".into(),
            body: "The clipboard holds no text.".into(),
        };
    };
    match lexiswitch_core::convert::convert(&text, from, to) {
        // Excluded: this is text the program derived, and Clipboard History persists what it
        // is shown. The restore path in `selection` deliberately does not exclude, because
        // what it puts back is the user's own.
        Some(converted) => match clip.set_text_excluded(&converted) {
            Ok(()) => Outcome::Note {
                title: "Clipboard converted".into(),
                body: format!("Converted to {} in place. Nothing was pasted.", to.name()),
            },
            Err(e) => Outcome::Failed {
                title: "Clipboard not converted".into(),
                body: e.to_string(),
            },
        },
        None => Outcome::Failed {
            title: "Clipboard not converted".into(),
            body: "Converting would not change the text.".into(),
        },
    }
}
