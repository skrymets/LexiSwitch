//! The conversion transaction, end to end, with no chooser in sight.
//!
//! This is the milestone that has to hold before any interface is worth building: read a
//! selection, lose the foreground to another window for as long as a user would spend choosing,
//! take it back, and replace the original with a candidate. If that round trip does not work
//! there is no point drawing anything.
//!
//!   cargo run -p lexiswitch-win --example probe_transaction
//!   cargo run -p lexiswitch-win --example probe_transaction -- --cancel
//!
//! The stand-in window is a real top-level window owned by this process, exactly as the chooser
//! will be — whether the foreground can be handed back depends on *who is asking*, so borrowing
//! someone else's window would measure a different question.
//!
//! Checks, in the order they matter:
//!
//! 1. the transaction reads the selection while the target still has focus
//! 2. candidates are generated from that text alone
//! 3. taking and returning the foreground leaves the selection intact
//! 4. commit replaces it, cancel does not
//! 5. the clipboard is what it was beforehand, either way
//!
//! Opens its own document. Refuses to type into anything else.

use std::io::Write;
use std::{thread, time::Duration};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

use lexiswitch_core::candidates;
use lexiswitch_win::clipboard::Clipboard;
use lexiswitch_win::focus::{process_name, Uia};
use lexiswitch_win::inject::{self, FocusGuard};
use lexiswitch_win::layouts;
use lexiswitch_win::transaction::Transaction;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A,
    VK_CONTROL, VK_N,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    PeekMessageW, RegisterClassW, SetForegroundWindow, ShowWindow, TranslateMessage, MSG,
    PM_REMOVE, SW_SHOW, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW,
};

/// Typed with the Ukrainian layout active while intending English.
const SOURCE: &str = "Еуче шт Утпдшір";
const PRIOR_CLIPBOARD: &str = "lexiswitch-prior-clipboard";
const CLASS_NAME: PCWSTR = w!("LexiSwitchTransactionStandIn");

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn chord(vk: VIRTUAL_KEY) {
    let inputs = [
        key(VK_CONTROL, false),
        key(vk, false),
        key(vk, true),
        key(VK_CONTROL, true),
    ];
    // SAFETY: a live slice of fully-initialised INPUT structures with the element size the API
    // requires.
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn set_clipboard(text: &str) -> bool {
    Clipboard::open(HWND(std::ptr::null_mut()), 5)
        .ok()
        .and_then(|c| c.set_text(text).ok())
        .is_some()
}

fn clipboard_text() -> Option<String> {
    Clipboard::open(HWND(std::ptr::null_mut()), 5).ok()?.text()
}

/// Read the whole document. Uses the clipboard, so it must come after any clipboard check.
fn document() -> String {
    chord(VK_A);
    thread::sleep(Duration::from_millis(150));
    let _ = inject::copy_selection();
    thread::sleep(Duration::from_millis(250));
    clipboard_text()
        .map(|t| t.trim_end_matches(['\r', '\n']).to_string())
        .unwrap_or_default()
}

/// A real top-level window, standing in for the chooser.
struct StandIn {
    hwnd: HWND,
}

impl StandIn {
    fn new() -> Option<Self> {
        // SAFETY: the class name is a static wide string with a NUL and the WNDCLASSW is fully
        // initialised before registration. A zero return from RegisterClassW means the class is
        // already registered, which is harmless; CreateWindowExW is the real check.
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(None).ok()?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let _ = RegisterClassW(&class);
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS_NAME,
                w!("LexiSwitch chooser stand-in"),
                WS_OVERLAPPEDWINDOW,
                200,
                200,
                560,
                220,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .ok()?
        };
        Some(Self { hwnd })
    }

    fn take_foreground(&self) {
        // SAFETY: `hwnd` was created by this type and is still alive.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.pump(400);
    }

    fn pump(&self, ms: u64) {
        for _ in 0..(ms / 20).max(1) {
            let mut msg = MSG::default();
            // SAFETY: `msg` is a valid MSG for the whole call.
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for StandIn {
    fn drop(&mut self) {
        // SAFETY: `hwnd` was created by this type and has not been destroyed elsewhere.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// SAFETY (contract on the caller, which is Windows): invoked by the window manager for a
/// window of this class. Nothing here dereferences `wp` or `lp`.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn check(label: &str, ok: bool, detail: &str) -> bool {
    say!("  {}  {label:<44}{detail}", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() {
    let cancelling = std::env::args().any(|a| a == "--cancel");
    let installed = match layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            say!("could not build layout tables: {e}");
            return;
        }
    };
    let uia = Uia::new();

    say!(
        "probe_transaction — {} path",
        if cancelling { "cancel" } else { "commit" }
    );
    if std::process::Command::new("notepad.exe").spawn().is_err() {
        say!("could not launch notepad.exe");
        return;
    }
    for _ in 0..40 {
        // SAFETY: no preconditions; a null handle is a valid answer.
        if process_name(unsafe { GetForegroundWindow() }).eq_ignore_ascii_case("notepad.exe") {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    thread::sleep(Duration::from_millis(1500));
    chord(VK_N);
    thread::sleep(Duration::from_millis(1200));

    // Seed by pasting: typing it is the mechanism this project retired.
    let guard = FocusGuard::capture();
    if !set_clipboard(SOURCE) || inject::paste(&guard).is_err() {
        say!("could not seed the document");
        return;
    }
    thread::sleep(Duration::from_millis(400));
    if document() != SOURCE {
        say!("the seeded text did not arrive; the setup is unsound, not the transaction");
        return;
    }
    chord(VK_A);
    thread::sleep(Duration::from_millis(250));
    if !set_clipboard(PRIOR_CLIPBOARD) {
        say!("could not set the prior clipboard");
        return;
    }

    say!();
    let mut ok = true;

    // 1. Read, while the target still has focus.
    let transaction = match Transaction::begin(
        uia.as_ref().and_then(|u| u.automation()),
        HWND(std::ptr::null_mut()),
        true,
        5,
    ) {
        Ok(t) => t,
        Err(e) => {
            say!("  FAIL  begin: {e}");
            return;
        }
    };
    ok &= check(
        "read the selection before losing focus",
        transaction.original() == SOURCE,
        &format!("{:?} via {:?}", transaction.original(), transaction.via()),
    );

    // 2. Candidates come from that text and nothing else.
    let found = candidates::generate(transaction.original(), &tables(&installed), None);
    ok &= check(
        "candidates generated from the original",
        !found.is_empty(),
        &format!(
            "{} candidate(s), first {}",
            found.len(),
            found
                .first()
                .map(|c| format!("{}->{} {:?}", c.from, c.to, c.text))
                .unwrap_or_default()
        ),
    );
    let Some(chosen) = found.first() else {
        say!("  (nothing to commit)");
        return;
    };

    // 3. A chooser would now take the foreground for as long as the user needs.
    let Some(standin) = StandIn::new() else {
        say!("  FAIL  could not create the stand-in window");
        return;
    };
    standin.take_foreground();
    ok &= check(
        "stand-in took the foreground",
        process_name(unsafe { GetForegroundWindow() }).eq_ignore_ascii_case("probe_transaction.exe"),
        "a chooser would be reading arrow keys here",
    );
    standin.pump(1500);

    // 4. End the transaction.
    if cancelling {
        match transaction.cancel() {
            Ok(()) => ok &= check("cancel returned cleanly", true, ""),
            Err(e) => ok &= check("cancel returned cleanly", false, &e.to_string()),
        }
        thread::sleep(Duration::from_millis(400));
        ok &= check(
            "clipboard restored",
            clipboard_text().as_deref() == Some(PRIOR_CLIPBOARD),
            "",
        );
        ok &= check("document untouched", document() == SOURCE, "");
    } else {
        let expected = chosen.text.clone();
        match transaction.commit(&expected) {
            Ok(done) => ok &= check("commit returned cleanly", true, &format!("via {:?}", done.via)),
            Err(e) => ok &= check("commit returned cleanly", false, &e.to_string()),
        }
        thread::sleep(Duration::from_millis(600));
        ok &= check(
            "clipboard restored",
            clipboard_text().as_deref() == Some(PRIOR_CLIPBOARD),
            "",
        );
        let doc = document();
        ok &= check(
            "selection replaced with the candidate",
            doc == expected,
            &format!("{doc:?}"),
        );
    }

    say!();
    say!(
        "{}",
        if ok {
            "ROUND TRIP HOLDS — a chooser can be built on this."
        } else {
            "ROUND TRIP BROKEN — fix this before drawing anything."
        }
    );
    say!("Close the Notepad window without saving.");
}

fn tables(installed: &[layouts::InstalledLayout]) -> Vec<lexiswitch_core::layout::LayoutTable> {
    installed.iter().map(|l| l.table.clone()).collect()
}
