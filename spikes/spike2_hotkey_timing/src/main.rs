//! Spike 2 — is `WM_HOTKEY` timing good enough for multi-tap?
//!
//! Two questions:
//!
//! 1. Do `WM_HOTKEY` message timestamps resolve finely enough to tell a tap from a
//!    double-tap from a triple-tap, and is ADR-0009's 500 ms window workable?
//! 2. When another application already holds the hotkey, does `RegisterHotKey` fail
//!    *detectably*? ADR-0008 requires reporting the conflict, which needs it to be visible.
//!
//! Timestamps are read from `MSG.time` — the moment the message was posted — never from a
//! wall-clock call inside the handler. A handler read measures when this process got around
//! to looking, so scheduling delay in the message loop would corrupt the count.
//!
//! Throwaway. `unwrap` is fine here.
//!
//!   cargo run                    register Pause and report tap timings
//!   cargo run -- --prove-conflict  register Pause twice, to show what a conflict looks like

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE,
    MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_HOTKEY, WNDCLASSW,
};

const VK_PAUSE: u32 = 0x13;
const HOTKEY_ID: i32 = 1;
const MULTI_TAP_MS: u32 = 500; // ADR-0009's proposed default, the value under test

fn main() {
    let hwnd = create_message_window();

    if std::env::args().any(|a| a == "--prove-conflict") {
        prove_conflict(hwnd);
        return;
    }

    // SAFETY: hwnd is a live message-only window owned by this thread.
    match unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(0), VK_PAUSE) } {
        Ok(()) => println!("registered Pause as a global hotkey"),
        Err(e) => {
            // Task 4.5: this is the shape ADR-0008's error surfacing depends on.
            println!("RegisterHotKey FAILED: {e}");
            println!("  (something else already holds Pause — that is the conflict path)");
            return;
        }
    }

    println!();
    println!("tap Pause deliberately: single, then double, then triple.");
    println!("timestamps come from MSG.time, not from a clock read in the handler.");
    println!("Ctrl+C to stop.\n");

    let mut msg = MSG::default();
    let mut prev: Option<u32> = None;
    let mut run = 0u32;

    // SAFETY: msg is valid for the loop; GetMessageW returns 0 at WM_QUIT.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                let t = msg.time;
                let delta = prev.map(|p| t.wrapping_sub(p));
                run = match delta {
                    Some(d) if d <= MULTI_TAP_MS => run + 1,
                    _ => 1,
                };
                let label = match run {
                    1 => "single (new correction)".to_string(),
                    2 => "double (2nd candidate)".to_string(),
                    3 => "triple (3rd candidate)".to_string(),
                    // The real cycle wraps at the candidate count; this spike does not know
                    // the candidate list, so it just reports depth.
                    n => format!("tap {n} in run"),
                };
                match delta {
                    Some(d) => println!("WM_HOTKEY  time={t:>10}  delta={d:>5} ms  -> {label}"),
                    None => println!("WM_HOTKEY  time={t:>10}  delta=    - ms  -> {label}"),
                }
                prev = Some(t);
            }
            let _ = DispatchMessageW(&msg);
        }
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }
}

/// Task 4.5 — show what a conflict looks like, without needing a second application.
///
/// Registering the same hotkey twice from one process fails the same way a cross-process
/// collision does. That does not make the cross-process test unnecessary — it makes the
/// *failure signature* observable now, so there is something concrete to compare against.
fn prove_conflict(hwnd: HWND) {
    println!("== conflict proof ==");
    // SAFETY: hwnd is live; both calls target the same window and id.
    unsafe {
        match RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(0), VK_PAUSE) {
            Ok(()) => println!("  first  RegisterHotKey(Pause) -> ok"),
            Err(e) => {
                println!("  first  RegisterHotKey(Pause) -> FAILED: {e}");
                println!("  something else already holds Pause; that is itself the answer.");
                return;
            }
        }
        match RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(0), VK_PAUSE) {
            Ok(()) => println!("  second RegisterHotKey(Pause) -> ok  <- UNEXPECTED"),
            Err(e) => {
                println!("  second RegisterHotKey(Pause) -> FAILED: {e}");
                println!("  code {:?}", e.code());
                println!("  verdict: conflicts are detectable, and this is what one looks like");
            }
        }
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }
}

/// `WNDPROC` requires `extern "system"`; windows-rs exposes `DefWindowProcW` as a plain
/// Rust `unsafe fn`, so it cannot be handed over directly.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwarding the exact arguments Windows passed.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn create_message_window() -> HWND {
    // SAFETY: static class name; message-only window needs no size or position.
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: w!("spike2_hotkey_timing"),
            ..Default::default()
        };
        let atom = RegisterClassW(&class);
        assert!(atom != 0, "RegisterClassW failed");

        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(atom as usize as *const u16),
            w!("spike2"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    }
}
