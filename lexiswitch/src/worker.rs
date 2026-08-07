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

use lexiswitch_core::candidates::{self, Candidate};
use lexiswitch_core::layout::LayoutTable;
use lexiswitch_win::chooser::{self, Decision};
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
        /// Layout name to raw HKL, so an apply-and-switch can name a layout to Windows rather
        /// than only to the user. Carried rather than looked up here because resolving it
        /// needs the installed set, which lives on the message-loop thread.
        handles: Vec<(String, u32)>,
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
                handles,
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
                        if found.is_empty() {
                            // A real answer, and a different one from a failure: the selection
                            // was read fine, and no installed layout pair changes it. No window
                            // is opened, because there is nothing to choose between.
                            let cancelled = transaction.cancel();
                            Outcome::Failed {
                                title: "No conversion found".into(),
                                body: match cancelled {
                                    Ok(()) => "No installed keyboard layout converts the \
                                               selected text into anything different."
                                        .into(),
                                    Err(e) => format!(
                                        "No installed keyboard layout converts the selected \
                                         text into anything different. Additionally: {e}"
                                    ),
                                },
                            }
                        } else {
                            // The chooser blocks this thread until the user decides. That is
                            // why it runs here and not on the message loop: the tray has to
                            // stay alive while a chooser is open.
                            match chooser::show(
                                transaction.original(),
                                &found,
                                active.as_deref(),
                            ) {
                                Err(e) => {
                                    // The window could not take the foreground, so it could
                                    // never have been typed into. Nothing was modified.
                                    let _ = transaction.cancel();
                                    Outcome::Failed {
                                        title: "Chooser did not open".into(),
                                        body: e.to_string(),
                                    }
                                }
                                Ok(Decision::Cancel) => match transaction.cancel() {
                                    Ok(()) => Outcome::Quiet,
                                    Err(e) => Outcome::Failed {
                                        title: "Cancelled, with a loose end".into(),
                                        body: e.to_string(),
                                    },
                                },
                                Ok(Decision::Apply { index }) => {
                                    apply(transaction, &found[index], None)
                                }
                                Ok(Decision::ApplyAndSwitch { index }) => {
                                    let candidate = &found[index];
                                    let hkl = handles
                                        .iter()
                                        .find(|(name, _)| *name == candidate.to)
                                        .map(|(_, hkl)| *hkl);
                                    apply(transaction, candidate, hkl)
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

/// Replace the selection with `candidate`, optionally asking Windows for its target layout.
///
/// Kept apart from the message handling because the outcomes are the point: the spec requires
/// success, a side effect, a failed replacement and an unconfirmed layout switch to be four
/// visibly different things, and none of them may be reported as another.
fn apply(transaction: Transaction, candidate: &Candidate, switch_to: Option<u32>) -> Outcome {
    let label = format!("{} to {}", candidate.from, candidate.to);

    let committed = match transaction.commit(&candidate.text) {
        Ok(done) => done,
        // Never reported as success, and never partially: the transaction restores what it
        // can before returning the error.
        Err(e) => {
            return Outcome::Failed {
                title: "Selection not converted".into(),
                body: e.to_string(),
            }
        }
    };

    // Only after the replacement landed. Switching the layout of a conversion that failed
    // would leave the user typing in a language they did not ask for, on text that never
    // changed.
    let switch_note = match switch_to {
        None => None,
        Some(hkl) => match lexiswitch_win::langswitch::activate_for_foreground(hkl) {
            // Advisory by nature: the request is posted and an application may decline it
            // with no way for us to learn that it did. Saying "requested" rather than
            // "switched" is the honest form of a thing that cannot be confirmed.
            Ok(()) => Some(format!("Typing in {} was requested.", candidate.to)),
            Err(e) => Some(format!(
                "The text was converted, but the switch to {} failed: {e}",
                candidate.to
            )),
        },
    };

    let clipboard_note = committed.lost_nontext.then(|| {
        "The clipboard held an image or formatted content, which the conversion destroyed. \
         Text on it was restored."
            .to_string()
    });

    match (switch_note, clipboard_note) {
        (None, None) => Outcome::Quiet,
        (switch, clip) => Outcome::Note {
            title: format!("Converted, {label}"),
            body: [switch, clip]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
        },
    }
}
