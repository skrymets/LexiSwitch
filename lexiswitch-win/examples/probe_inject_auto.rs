//! Measure whether synthesised text survives the trip into an application, unattended.
//!
//! `probe_inject` needs a human to focus a target and read the result. This drives the same
//! experiment end to end: it launches its own Notepad, types a known string at several pacings,
//! reads back what actually arrived through UI Automation, and compares. No eye required, and
//! the answer is a diff rather than an impression.
//!
//!   cargo run -p lexiswitch-win --example probe_inject_auto
//!
//! The question it exists to settle: `inject::replace` pushes the whole replacement through a
//! single unpaced `SendInput`. An application that consumes input asynchronously can drop and
//! repeat characters under that, and `SendInput` still reports every event accepted — so the
//! program cannot currently tell. Varying only the pacing shows whether that is the cause.
//!
//! Leaves a Notepad window holding the last trial's text. Close it without saving.

use std::io::Write;
use std::{thread, time::Duration};

/// Print and flush.
///
/// Output goes to a pipe as often as to a terminal, and a block-buffered pipe turns a stall
/// into silence — which is indistinguishable from the probe having done nothing at all.
macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::PasswordGate;
use lexiswitch_win::inject::{self, FocusGuard, SYNTHETIC_MARKER};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N,
};

/// The real converted sentence from the Notepad report, so this reproduces the case under
/// investigation rather than a toy one.
const TEXT: &str = "Test if selected text may be transliterated back to the correct locale";

/// Pacings to compare. The first is exactly what `inject::replace` does today.
const TRIALS: &[(&str, usize, u64)] = &[
    ("one batch (as inject::replace)", usize::MAX, 0),
    ("8 chars per call, 2 ms apart", 8, 2),
    ("1 char per call, 5 ms apart", 1, 5),
    ("1 char per call, 15 ms apart", 1, 15),
];

/// Each pacing is run this many times. A single run of a race proves nothing; what matters is
/// how often it survives.
const REPEATS: usize = 5;

/// How long the target is given to drain its input queue before the readback.
///
/// Generous on purpose: a short settle makes a slow application look like a lossy one, and
/// that confound would point the fix at the wrong place.
const SETTLE: Duration = Duration::from_millis(1200);

/// Open a fresh empty document in the target before typing anything.
///
/// Not a nicety. An editor that restores its previous session comes up holding the user's own
/// files, and this probe selects-all and types — into whatever is in front of it. It types into
/// a document it created, or it does not type.
fn new_document() {
    chord(VK_N);
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: SYNTHETIC_MARKER as usize,
            },
        },
    }
}

/// Send Ctrl+<vk>.
fn chord(vk: VIRTUAL_KEY) {
    let inputs = [
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ];
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires. Carries the synthetic marker, as everything this crate sends does.
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Select the whole document, so the next typed character replaces it.
fn select_all() {
    chord(VK_A);
}

/// Whether the target's document is empty, established without trusting the clipboard's prior
/// contents.
///
/// A plain select-all-and-copy cannot answer this: copying an empty selection leaves the
/// clipboard alone, so the previous contents come back and an empty document reads as full. A
/// sentinel is planted first, and surviving the copy is what proves nothing was selected.
fn document_is_empty() -> Emptiness {
    const SENTINEL: &str = "\u{1}lexiswitch-probe-sentinel";

    let planted = Clipboard::open(HWND(std::ptr::null_mut()), 5)
        .ok()
        .and_then(|clip| clip.set_text(SENTINEL).ok())
        .is_some();
    if !planted {
        // Unable to establish the precondition, so the safe answer is the refusing one.
        return Emptiness::Unknown("could not plant the sentinel on the clipboard");
    }

    select_all();
    thread::sleep(Duration::from_millis(150));
    if inject::copy_selection().is_err() {
        return Emptiness::Unknown("could not send the copy chord");
    }
    thread::sleep(Duration::from_millis(250));

    match Clipboard::open(HWND(std::ptr::null_mut()), 5)
        .ok()
        .and_then(|c| c.text())
    {
        // The sentinel survived, so the copy found nothing to overwrite it with.
        Some(text) if text == SENTINEL => Emptiness::Empty,
        // An empty document is not always an empty copy: some editors yield the blank line's
        // terminator. Whitespace is nothing worth refusing to overwrite.
        Some(text) if text.trim().is_empty() => Emptiness::Empty,
        // Lengths only. Whatever is in the target belongs to the user, not to this log.
        Some(text) => Emptiness::Occupied(text.chars().count()),
        None => Emptiness::Unknown("the clipboard came back holding no text at all"),
    }
}

/// What the emptiness check saw, kept separate so a refusal can say which reason it had.
enum Emptiness {
    Empty,
    Occupied(usize),
    Unknown(&'static str),
}

/// Read the target's document back through the clipboard.
///
/// Deliberately not UI Automation: the editors worth comparing include ones that expose no text
/// pattern at all, and a readback that works on one target and not the other measures nothing.
/// Ctrl+A then Ctrl+C is four events, orders of magnitude less input than the payload under
/// test, so it is not a meaningful source of the loss being measured.
fn read_back() -> Option<String> {
    select_all();
    thread::sleep(Duration::from_millis(150));
    inject::copy_selection().ok()?;
    thread::sleep(Duration::from_millis(250));
    let clip = Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?;
    clip.text()
}

/// Wait until the named process owns the foreground.
///
/// Matched exactly rather than by prefix, because `notepad.exe` and `notepad++.exe` are the
/// two targets being compared and one must never be mistaken for the other.
fn wait_for_target(gate: &mut PasswordGate, process: &str) -> bool {
    for _ in 0..40 {
        gate.invalidate();
        if gate.current().process.eq_ignore_ascii_case(process) {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn inject_paced(guard: &FocusGuard, text: &str, chunk: usize, delay: u64) -> Result<(), String> {
    let chars: Vec<char> = text.chars().collect();
    let size = chunk.min(chars.len().max(1));
    for piece in chars.chunks(size) {
        let piece: String = piece.iter().collect();
        inject::replace(guard, 0, &piece).map_err(|e| e.to_string())?;
        if delay > 0 {
            thread::sleep(Duration::from_millis(delay));
        }
    }
    Ok(())
}

/// Where the two strings first diverge, in characters.
fn first_difference(want: &str, got: &str) -> Option<usize> {
    want.chars()
        .zip(got.chars())
        .position(|(a, b)| a != b)
        .or_else(|| (want.chars().count() != got.chars().count()).then(|| want.chars().count()))
}

fn main() {
    let mut gate = PasswordGate::new();
    if !gate.has_automation() {
        say!("UI Automation unavailable — this probe cannot read results back.");
        return;
    }

    // Defaults to modern Notepad; pass a path to compare another editor against it.
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "notepad.exe".to_string());
    let process = std::path::Path::new(&target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.clone());

    say!("probe_inject_auto — launching {target}");
    if let Err(e) = std::process::Command::new(&target).spawn() {
        say!("could not launch {target}: {e}");
        return;
    }
    if !wait_for_target(&mut gate, &process) {
        say!("{process} did not reach the foreground; nothing was typed.");
        return;
    }
    // The editor takes focus a moment after the window appears.
    thread::sleep(Duration::from_millis(1500));
    // Never type into whatever the target restored from its last session. Retried, because a
    // just-launched editor may not be listening for the chord yet.
    let mut emptiness = Emptiness::Unknown("not yet checked");
    for attempt in 0..3 {
        new_document();
        thread::sleep(Duration::from_millis(1200));
        emptiness = document_is_empty();
        if matches!(emptiness, Emptiness::Empty) {
            break;
        }
        say!("  new-document attempt {} did not land", attempt + 1);
    }
    match emptiness {
        Emptiness::Empty => {}
        Emptiness::Occupied(n) => {
            say!(
                "{process} is showing a document of {n} characters, not an empty one. Nothing was \
                 typed — it would have overwritten whatever that is."
            );
            return;
        }
        Emptiness::Unknown(why) => {
            say!("could not establish that {process} is showing an empty document: {why}.");
            say!("Nothing was typed.");
            return;
        }
    }

    say!("expected: {TEXT:?} ({} chars)", TEXT.chars().count());
    say!();

    say!("{REPEATS} runs per pacing, {} ms settle before readback.", SETTLE.as_millis());
    say!();

    for (label, chunk, delay) in TRIALS {
        let mut survived = 0usize;
        // Distinguishes a lost character mid-string from a tail that never arrived: the former
        // is real mangling, the latter can also mean the readback was simply early.
        let mut dropped_inside = 0usize;
        let mut worst: Option<String> = None;

        for _ in 0..REPEATS {
            select_all();
            thread::sleep(Duration::from_millis(150));

            let guard = FocusGuard::capture();
            if let Err(e) = inject_paced(&guard, TEXT, *chunk, *delay) {
                say!("{label:<32} ABANDONED  {e}");
                break;
            }
            thread::sleep(SETTLE);

            match read_back() {
                None => println!("{label:<32} UNREADABLE (no text pattern)"),
                Some(got) => {
                    let got = got.trim_end_matches(['\r', '\n']).to_string();
                    if got == TEXT {
                        survived += 1;
                    } else {
                        if first_difference(TEXT, &got).is_some_and(|n| n < TEXT.chars().count()) {
                            dropped_inside += 1;
                        }
                        if worst.is_none() {
                            worst = Some(got);
                        }
                    }
                }
            }
        }

        say!(
            "{label:<32} {survived}/{REPEATS} intact   ({dropped_inside} mangled mid-string, \
             rest short)"
        );
        if let Some(sample) = worst {
            say!("{:>34}{sample:?}", "");
        }
    }

    say!();
    say!("Close the Notepad window without saving.");
}
