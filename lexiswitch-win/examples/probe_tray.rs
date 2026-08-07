//! Does the real tray code receive clicks, or only a hand-rolled icon?
//!
//! An earlier version of this probe added icons with raw `Shell_NotifyIcon` calls and they
//! received right-clicks normally, while the application's icon received nothing. That was a
//! useless comparison: the probe was minimal, so the difference between it and the application
//! was *everything the application does*, and the one thing that mattered was hidden among the
//! rest. A differential test is only as good as the differences it actually reproduces.
//!
//! This puts both on the same footing. One window — the product's own [`MessageWindow`] —
//! carries two icons:
//!
//! | uID | Added by | Modified afterwards |
//! |-----|----------|---------------------|
//! | 1   | the product's [`Tray`], then a balloon | yes, `NIM_MODIFY` |
//! | 99  | raw `NIM_ADD` here, nothing else       | no |
//!
//! Same process, same thread, same window, same message loop, same callback message. The only
//! difference left is the code that registers and then modifies the icon.
//!
//!   cargo run -p lexiswitch-win --example probe_tray
//!
//! Right-click **both** icons. Each callback is logged with the id that produced it, so the
//! answer is which ids appear:
//!
//! - **both** — the tray code is fine and the fault is elsewhere in the application
//! - **99 only** — the product's registration path is what loses the callback
//! - **neither** — the window or the loop is at fault, not the icon

use std::io::Write;

use lexiswitch_core::state::{AgentState, Shortcut, Shortcuts};
use lexiswitch_win::tray::{Tray, WM_TRAY};
use lexiswitch_win::window::{Flow, MessageWindow};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, IDI_WARNING};

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

const CONTROL_ID: u32 = 99;

/// A second icon on the same window, added once and never touched again.
fn add_control(hwnd: HWND) -> bool {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: CONTROL_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        ..Default::default()
    };
    // SAFETY: a built-in resource identifier needs no module handle, and `data` is fully
    // initialised with a matching cbSize.
    unsafe {
        data.hIcon = LoadIconW(None, IDI_WARNING).expect("built-in icon");
        for (i, ch) in "CONTROL raw add, never modified"
            .encode_utf16()
            .take(127)
            .enumerate()
        {
            data.szTip[i] = ch;
        }
        Shell_NotifyIconW(NIM_ADD, &data).as_bool()
    }
}

fn remove_control(hwnd: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: CONTROL_ID,
        ..Default::default()
    };
    // SAFETY: removing an icon this process added.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn main() {
    let window = MessageWindow::new().expect("message window");
    say!("probe_tray — the product's tray code against a raw control, same window.");
    say!("  window: {:?}", window.hwnd().0);

    let state = AgentState::resolve(
        2,
        Shortcuts {
            convert: Shortcut::Registered("Shift+Pause".into()),
            case: Shortcut::Registered("Win+Ctrl+Pause".into()),
        },
        true,
        None,
    );
    let tray = Tray::new(window.hwnd(), &state).expect("tray");
    say!("  uID 1  added by the product's Tray");

    // The application shows a balloon immediately after adding its icon, so the probe does
    // too. This is the call under suspicion: it is a NIM_MODIFY, and whether it costs the
    // icon its callback is exactly what this run is meant to decide.
    tray.notify("probe_tray", "balloon sent, exactly as the product does at startup");
    say!("  uID 1  balloon sent (NIM_MODIFY)");

    say!("  uID 99 added raw, and will not be modified: {}", add_control(window.hwnd()));
    say!();
    say!("Right-click BOTH icons, then close this window.");
    say!("  'PRODUCT via Tray'            is uID 1");
    say!("  'CONTROL raw add, never...'   is uID 99");
    say!();

    let _ = window.run(|msg| {
        if msg.message == WM_TRAY {
            say!(
                "  callback  uID={}  event=0x{:04X}",
                msg.wParam.0 as u32,
                msg.lParam.0 as u32
            );
        }
        Flow::Continue
    });

    remove_control(window.hwnd());
}
