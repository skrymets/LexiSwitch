//! The worker thread: everything that is allowed to be slow.
//!
//! The message-loop thread runs the window, hotkeys and the tray, and must never block — a
//! stalled loop means a dead tray icon. UI Automation and clipboard calls can take seconds,
//! so they live here instead, reached over a channel.
//!
//! No COM interface crosses the boundary. UI Automation is apartment-bound, so this thread
//! initializes its own rather than accepting one built elsewhere.

use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use lexiswitch_core::layout::LayoutTable;
use lexiswitch_win::focus::Uia;
use lexiswitch_win::selection;
use lexiswitch_win::window::Waker;

/// Work handed to the worker. Everything here is owned data — nothing borrows from the
/// message-loop thread, and no handle that is thread-affine crosses.
// One job is sent per hotkey press, so the variants' size disparity costs a few hundred bytes
// at human speed. Boxing the tables would add indirection to buy nothing measurable.
#[allow(clippy::large_enum_variant)]
pub enum Job {
    /// Convert whatever is selected, automation first.
    Selection {
        from: LayoutTable,
        to: LayoutTable,
        allow_clipboard: bool,
    },
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
    ///
    /// `waker` wakes the message loop when an outcome is ready. Without it nothing is ever
    /// reported: the loop blocks in `GetMessageW`, so a result produced while it is idle sits
    /// unread until something unrelated happens to wake it.
    pub fn spawn(waker: Waker) -> (Self, Receiver<Outcome>) {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let (out_tx, out_rx) = std::sync::mpsc::channel::<Outcome>();

        let handle = std::thread::Builder::new()
            .name("lexiswitch-worker".into())
            .spawn(move || run(rx, out_tx, waker))
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

fn run(rx: Receiver<Job>, out: Sender<Outcome>, waker: Waker) {
    // This thread's own automation object. COM apartments are per-thread and an interface
    // created elsewhere must not be used here, so it is built rather than borrowed.
    let uia = Uia::new();

    while let Ok(job) = rx.recv() {
        let outcome = match job {
            Job::Stop => break,
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
        };

        if out.send(outcome).is_ok() {
            // Drop the result into the channel, then wake the loop so it actually looks.
            waker.wake();
        } else {
            // The message loop has gone; nothing left to report to.
            break;
        }
    }
}
