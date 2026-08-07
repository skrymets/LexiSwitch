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

use lexiswitch_core::candidates;
use lexiswitch_core::layout::LayoutTable;
use lexiswitch_win::focus::Uia;
use lexiswitch_win::transaction::Transaction;
use lexiswitch_win::window::Waker;

/// Work handed to the worker. Everything here is owned data — nothing borrows from the
/// message-loop thread, and no handle that is thread-affine crosses.
// One job is sent per hotkey press, so the variants' size disparity costs a few hundred bytes
// at human speed. Boxing the tables would add indirection to buy nothing measurable.
#[allow(clippy::large_enum_variant)]
pub enum Job {
    /// Convert whatever is selected.
    ///
    /// Carries every installed layout rather than a chosen pair: which conversions are
    /// plausible is decided from the selected characters, not from what happens to be active
    /// ([ADR-0012](../../docs/adr/0012-single-chooser-transaction.md)).
    Selection {
        tables: Vec<LayoutTable>,
        /// The layout active in the focused window, as a weak tiebreak on the *target* only.
        /// It is never allowed to choose a source.
        active: Option<String>,
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
                tables,
                active,
                allow_clipboard,
            } => {
                // No owner window: the worker owns none, and the clipboard does not need one.
                let owner = windows::Win32::Foundation::HWND(std::ptr::null_mut());
                match Transaction::begin(
                    uia.as_ref().and_then(|u| u.automation()),
                    owner,
                    allow_clipboard,
                    5,
                ) {
                    Err(e) => Outcome::Failed {
                        title: "Selection not converted".into(),
                        body: e.to_string(),
                    },
                    Ok(transaction) => {
                        let found = candidates::generate(
                            transaction.original(),
                            &tables,
                            active.as_deref(),
                        );
                        match found.first() {
                            // A real answer, and a different one from a failure: the selection
                            // was read fine, and no installed layout pair changes it.
                            None => {
                                let cancelled = transaction.cancel();
                                Outcome::Failed {
                                    title: "No conversion found".into(),
                                    body: match cancelled {
                                        Ok(()) => "No installed keyboard layout converts the                                                    selected text into anything different."
                                            .into(),
                                        Err(e) => format!(
                                            "No installed keyboard layout converts the selected                                              text into anything different. Additionally: {e}"
                                        ),
                                    },
                                }
                            }
                            // Until the chooser exists, the highest-ranked candidate is applied
                            // directly. That is deliberately temporary: the pivot requires that
                            // nothing is modified before an explicit commit, and this is the
                            // one part of that promise not yet kept.
                            Some(best) => {
                                let label = format!("{} to {}", best.from, best.to);
                                match transaction.commit(&best.text) {
                                    Ok(done) if done.lost_nontext => Outcome::Note {
                                        title: format!("Converted, {label}"),
                                        body: "The clipboard held an image or formatted                                                content, which the conversion destroyed. Text                                                on it was restored."
                                            .into(),
                                    },
                                    Ok(_) => Outcome::Quiet,
                                    Err(e) => Outcome::Failed {
                                        title: "Selection not converted".into(),
                                        body: e.to_string(),
                                    },
                                }
                            }
                        }
                    }
                }
            }
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
