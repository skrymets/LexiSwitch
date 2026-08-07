//! Which tray-icon configuration does the shell actually talk to?
//!
//! Three fixes to the tray menu in a row each addressed a real defect and none of them made a
//! right-click work, because each was verified on the half of the problem that could be
//! reached from inside the process. Posting the callback message by hand opens the menu; the
//! shell never sends one. So the fault is in how the icon is registered, and the only way to
//! find it is to register several ways at once and see which the shell speaks to.
//!
//!   cargo run -p lexiswitch-win --example probe_tray
//!
//! Adds three icons, then reports every callback the shell delivers:
//!
//! | Icon | Window       | Protocol |
//! |------|--------------|----------|
//! | ⚠    | message-only | legacy   |
//! | ?    | top-level    | legacy   |
//! | 🛈    | top-level    | version 4 |
//!
//! Hover over each, then left-click, then right-click. A line per event names which icon it
//! came from. An icon that produces no lines is one the shell is not talking to, and an icon
//! with no tooltip is the same finding by another route — the tooltip needs no message loop,
//! so losing it means the registration itself is wrong rather than the handling.
//!
//! Ctrl+C to stop. Icons are removed on the way out.

use std::io::Write;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadIconW, RegisterClassW,
    HWND_MESSAGE, IDI_ASTERISK, IDI_QUESTION, IDI_WARNING, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const CALLBACK: u32 = WM_APP + 1;

macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

/// One registration under test.
struct Variant {
    id: u32,
    label: &'static str,
    v4: bool,
}

const VARIANTS: [Variant; 3] = [
    Variant { id: 1, label: "A  message-only window, legacy protocol", v4: false },
    Variant { id: 2, label: "B  top-level window,    legacy protocol", v4: false },
    Variant { id: 3, label: "C  top-level window,    version 4", v4: true },
];

fn make_window(class: PCWSTR, top_level: bool) -> HWND {
    // SAFETY: the class name is a static wide string with a NUL and the WNDCLASSW is fully
    // initialised before registration. A zero return from RegisterClassW means the class is
    // already registered, which is harmless; CreateWindowExW is the real check.
    unsafe {
        let hinstance = GetModuleHandleW(None).expect("module handle");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
        CreateWindowExW(
            if top_level {
                WS_EX_TOOLWINDOW
            } else {
                WINDOW_EX_STYLE(0)
            },
            class,
            w!("probe_tray"),
            if top_level {
                WS_POPUP
            } else {
                WINDOW_STYLE(0)
            },
            0,
            0,
            0,
            0,
            if top_level { None } else { Some(HWND_MESSAGE) },
            None,
            Some(hinstance.into()),
            None,
        )
        .expect("creating the window")
    }
}

fn add_icon(hwnd: HWND, v: &Variant, tip: &str) {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: v.id,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: CALLBACK,
        Anonymous: NOTIFYICONDATAW_0 {
            uVersion: if v.v4 { NOTIFYICON_VERSION_4 } else { 0 },
        },
        ..Default::default()
    };

    // SAFETY: built-in resource identifiers need no module handle, and `data` is fully
    // initialised with a matching cbSize before every call.
    unsafe {
        let icon = match v.id {
            1 => IDI_WARNING,
            2 => IDI_QUESTION,
            _ => IDI_ASTERISK,
        };
        data.hIcon = LoadIconW(None, icon).expect("built-in icon");
        for (i, ch) in tip.encode_utf16().take(127).enumerate() {
            data.szTip[i] = ch;
        }

        let added = Shell_NotifyIconW(NIM_ADD, &data).as_bool();
        let versioned = if v.v4 {
            Shell_NotifyIconW(NIM_SETVERSION, &data).as_bool()
        } else {
            true
        };
        say!("  {}   NIM_ADD={added}  NIM_SETVERSION={versioned}", v.label);
    }
}

fn remove_icon(hwnd: HWND, id: u32) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: id,
        ..Default::default()
    };
    // SAFETY: removing an icon this process added, with a matching cbSize.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// SAFETY (contract on the caller, which is Windows): invoked by the window manager for a
/// window of one of these classes. Nothing here dereferences `wp` or `lp`.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == CALLBACK {
        // Decoded both ways, because which is correct is the question. Legacy puts the icon id
        // in wParam and the event in lParam; version 4 puts the cursor in wParam and packs the
        // event into the low word of lParam with the id in the high word.
        let legacy_id = wp.0 as u32;
        let legacy_event = lp.0 as u32;
        let v4_event = (lp.0 as u32) & 0xFFFF;
        let v4_id = ((lp.0 as u32) >> 16) & 0xFFFF;
        say!(
            "    callback  wParam=0x{:08X} lParam=0x{:08X}   legacy(id={legacy_id} event=0x{legacy_event:04X})   v4(id={v4_id} event=0x{v4_event:04X})",
            wp.0 as u32,
            lp.0 as u32
        );
    }
    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn main() {
    let msg_only = make_window(w!("ProbeTrayMessageOnly"), false);
    let top_level = make_window(w!("ProbeTrayTopLevel"), true);
    say!("probe_tray — three registrations, one shell.");
    say!("  message-only window: {:?}", msg_only.0);
    say!("  top-level window:    {:?}", top_level.0);
    say!();

    add_icon(msg_only, &VARIANTS[0], "A message-only legacy");
    add_icon(top_level, &VARIANTS[1], "B top-level legacy");
    add_icon(top_level, &VARIANTS[2], "C top-level v4");

    say!();
    say!("Now hover, left-click and right-click each of the three new icons.");
    say!("Each line below names the icon by id: 1=A, 2=B, 3=C.");
    say!("An icon that never appears below is one the shell is not talking to.");
    say!("Tooltips tell the same story without any messages at all — an icon with no");
    say!("tooltip is mis-registered rather than mis-handled.");
    say!();

    let mut msg = MSG::default();
    // SAFETY: `msg` is valid for the whole call; GetMessageW returns 0 for WM_QUIT.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = DispatchMessageW(&msg);
        }
    }

    remove_icon(msg_only, 1);
    remove_icon(top_level, 2);
    remove_icon(top_level, 3);
}
