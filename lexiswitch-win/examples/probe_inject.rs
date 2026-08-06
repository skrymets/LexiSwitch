//! Type text over the current selection, with the injection paced to order.
//!
//! This isolates the *write* half of a selection conversion. `convert_selection` replaces a
//! selection the same way on both of its paths — by synthesising the replacement text, because
//! typing over a selection replaces it — so when a conversion produces mangled output, the
//! question is whether the text was converted wrongly or delivered wrongly.
//!
//! `SendInput` reports how many events it inserted into the input stream, not how many the
//! target acted on, so "all accepted" is not evidence that anything arrived intact. This probe
//! exists to put a human eye on the difference, and to vary the one thing `inject::replace`
//! does not currently vary: how fast the events go out.
//!
//!   cargo run -p lexiswitch-win --example probe_inject                      # one batch, as today
//!   cargo run -p lexiswitch-win --example probe_inject -- --chunk 1 --delay 5
//!   cargo run -p lexiswitch-win --example probe_inject -- --text "short"
//!
//! `--chunk N` sends N characters per `SendInput` call; the default sends everything in one
//! call, which is exactly what `inject::replace` does. `--delay MS` waits between chunks.
//!
//! Focus the target and select some text during the countdown. Do not aim it at anything whose
//! contents matter.

use std::{thread, time::Duration};

use lexiswitch_win::inject::{self, FocusGuard};

/// The real converted sentence from the Notepad report, so the default run reproduces the
/// case under investigation rather than a toy one.
const DEFAULT_TEXT: &str = "Test if selected text may be transliterated back to the correct locale";

struct Args {
    text: String,
    chunk: usize,
    delay: u64,
}

fn parse() -> Args {
    let mut args = Args {
        text: DEFAULT_TEXT.to_string(),
        chunk: usize::MAX,
        delay: 0,
    };
    let mut rest = std::env::args().skip(1);
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--text" => {
                if let Some(v) = rest.next() {
                    args.text = v;
                }
            }
            "--chunk" => {
                if let Some(v) = rest.next().and_then(|v| v.parse().ok()) {
                    args.chunk = v;
                }
            }
            "--delay" => {
                if let Some(v) = rest.next().and_then(|v| v.parse().ok()) {
                    args.delay = v;
                }
            }
            other => eprintln!("ignoring unrecognised argument {other:?}"),
        }
    }
    if args.chunk == 0 {
        args.chunk = 1;
    }
    args
}

fn main() {
    let args = parse();
    let chars: Vec<char> = args.text.chars().collect();
    let chunks: Vec<String> = chars
        .chunks(args.chunk.min(chars.len().max(1)))
        .map(|c| c.iter().collect())
        .collect();

    println!("probe_inject — replaces the current selection.");
    println!("  text   {:?} ({} chars)", args.text, chars.len());
    println!(
        "  pacing {}",
        if chunks.len() == 1 {
            "one SendInput call — identical to inject::replace".to_string()
        } else {
            format!(
                "{} calls of up to {} chars, {} ms apart",
                chunks.len(),
                args.chunk,
                args.delay
            )
        }
    );
    println!("Focus the target application and select some text now.");
    for remaining in (1..=5).rev() {
        println!("  {remaining}...");
        thread::sleep(Duration::from_secs(1));
    }

    // Captured after the countdown, so the guard names the target the user just focused rather
    // than this console.
    let guard = FocusGuard::capture();
    for (n, chunk) in chunks.iter().enumerate() {
        if let Err(e) = inject::replace(&guard, 0, chunk) {
            println!("chunk {n} not injected: {e}");
            return;
        }
        if args.delay > 0 {
            thread::sleep(Duration::from_millis(args.delay));
        }
    }

    println!();
    println!("Windows accepted every event. Now compare the target against the text above.");
    println!("Characters repeated or missing means the application received the events and");
    println!("mangled them — which a conversion cannot currently detect.");
}
