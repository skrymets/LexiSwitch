//! Spike 1 — does Raw Input reach into elevated windows?
//!
//! Question: does `RIDEV_INPUTSINK` deliver keystrokes typed into an *elevated* window to an
//! *unelevated* process? Low-level hooks do not cross that boundary (UIPI). If Raw Input
//! matches the restriction it is an accepted limitation of ADR-0003, but the project should
//! learn that now rather than after shipping.
//!
//! This spike cannot answer its own question — it prints what it observes and a human reads
//! the output. Run it unelevated, type into Notepad, then type into an elevated window.
//!
//! Throwaway. `unwrap` is fine here.

use std::ffi::c_void;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, RegisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_INPUT, WNDCLASSW,
};

fn main() {
    println!("spike 1 — RIDEV_INPUTSINK reach");
    println!("this process is {}", elevation());
    println!();
    println!("1. type into Notepad (unelevated)      -> events should appear below");
    println!("2. type into an ELEVATED Notepad/Terminal -> do events still appear?");
    println!("   that second answer is the whole question.");
    println!();
    println!("Ctrl+C to stop.\n");

    let hwnd = create_message_window();
    register_raw_keyboard(hwnd);

    let mut msg = MSG::default();
    // SAFETY: msg is a valid MSG for the duration of the loop; GetMessageW returns 0 on
    // WM_QUIT and -1 on error, both of which end the loop.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_INPUT {
                on_raw_input(HRAWINPUT(msg.lParam.0 as *mut c_void));
            }
            // Still dispatched: DefWindowProcW performs the cleanup WM_INPUT requires.
            let _ = DispatchMessageW(&msg);
        }
    }
}

/// `WNDPROC` requires `extern "system"`, and windows-rs exposes `DefWindowProcW` as a plain
/// Rust `unsafe fn`, so it cannot be handed over directly. This forwards everything.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwarding the exact arguments Windows passed to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn create_message_window() -> HWND {
    // SAFETY: the class name is a static wide string; the window is message-only, so it is
    // never shown and needs no size or position.
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: w!("spike1_uipi_reach"),
            ..Default::default()
        };
        let atom = RegisterClassW(&class);
        assert!(atom != 0, "RegisterClassW failed");

        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(atom as usize as *const u16),
            w!("spike1"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE), // message-only: no taskbar entry, no painting, still gets messages
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    }
}

fn register_raw_keyboard(hwnd: HWND) {
    let dev = RAWINPUTDEVICE {
        usUsagePage: 0x01, // generic desktop
        usUsage: 0x06,     // keyboard
        // INPUTSINK: deliver input even when this window is not in the foreground. This is
        // the observe-only capability ADR-0003 relies on — it cannot block or alter input.
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    // SAFETY: one device descriptor, size passed as the API requires.
    unsafe {
        RegisterRawInputDevices(&[dev], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
            .expect("RegisterRawInputDevices failed");
    }
    println!("registered for keyboard raw input (RIDEV_INPUTSINK)\n");
}

fn on_raw_input(handle: HRAWINPUT) {
    let mut raw = RAWINPUT::default();
    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
    let header = std::mem::size_of::<RAWINPUTHEADER>() as u32;

    // SAFETY: raw is a real RAWINPUT and size is its true size; GetRawInputData writes at
    // most that many bytes and returns the count it wrote.
    let n = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(&mut raw as *mut _ as *mut c_void),
            &mut size,
            header,
        )
    };
    if n == u32::MAX || raw.header.dwType != RIM_TYPEKEYBOARD.0 {
        return;
    }

    // SAFETY: dwType said keyboard, so the keyboard arm of the union is the live one.
    let kb = unsafe { raw.data.keyboard };
    let updown = if kb.Flags & 0x01 == 0 { "down" } else { "up  " };

    println!(
        "scancode 0x{:02x}  vkey 0x{:02x}  flags 0x{:04x}  {}   foreground: {}",
        kb.MakeCode,
        kb.VKey,
        kb.Flags,
        updown,
        foreground_process()
    );
}

fn foreground_process() -> String {
    // SAFETY: every handle opened here is closed on both paths; the buffer length is passed
    // by reference as QueryFullProcessImageNameW expects.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return "<none>".into();
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            // Access denied here is itself informative: an elevated process is opaque to an
            // unelevated one, which is the same boundary this spike is testing.
            return format!("<pid {pid}, access denied>");
        };

        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let name = if QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            String::from_utf16_lossy(&buf[..len as usize])
                .rsplit('\\')
                .next()
                .unwrap_or("<?>")
                .to_string()
        } else {
            format!("<pid {pid}>")
        };
        let _ = CloseHandle(h);
        name
    }
}

fn elevation() -> &'static str {
    // Deliberately crude: this spike only needs to state what it is, and the accurate
    // answer comes from how the human launched it.
    if std::env::var("__COMPAT_LAYER").is_ok() {
        "possibly elevated"
    } else {
        "running as launched (check the title bar of your shell if unsure)"
    }
}
