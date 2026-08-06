//! What has keyboard focus.
//!
//! Two things survive here: the name of the process owning the foreground window, and the UI
//! Automation object used to read a selection.
//!
//! Password-field detection lived here until
//! [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md). It existed to keep the
//! keystroke buffer from filling in a password field; with nothing captured there is no
//! keystroke to withhold. Its coverage was measured rather than assumed, and was weak inside
//! browsers and Electron — removing the capture it guarded resolves that rather than papering
//! over it.

use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

/// The name of the process owning `hwnd`, without its path.
pub fn process_name(hwnd: HWND) -> String {
    // SAFETY: every opened handle is closed on both paths. QueryFullProcessImageNameW is
    // given a buffer and its length by reference, as documented.
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let name = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            String::from_utf16_lossy(&buf[..len as usize])
                .rsplit('\\')
                .next()
                .unwrap_or_default()
                .to_string()
        } else {
            String::new()
        };
        let _ = CloseHandle(handle);
        name
    }
}

/// UI Automation, created once and reused.
///
/// COM interaction is confined to this type. `IUIAutomation` is a reference-counted smart
/// pointer from windows-rs, so its lifetime is handled by `Drop` rather than by hand, and no
/// raw interface pointer escapes this module. It is deliberately not `Send`: a COM interface
/// created on this apartment must not cross to the worker thread.
pub struct Uia {
    automation: IUIAutomation,
}

impl Uia {
    /// The underlying interface, for callers that need it directly — reading a selection,
    /// for instance.
    ///
    /// Deliberately borrowed rather than cloned out: an interface obtained here belongs to
    /// this thread's apartment and must not be sent to another.
    pub fn automation(&self) -> Option<&IUIAutomation> {
        Some(&self.automation)
    }
}

impl Uia {
    /// Initialise COM for this thread and create the automation object.
    ///
    /// Returns `None` if either step fails, in which case detection falls back to the window
    /// style. A machine without working UI Automation should still run, with reduced
    /// coverage that is reported rather than hidden.
    pub fn new() -> Option<Self> {
        // SAFETY: apartment-threaded matches the message-loop thread this runs on.
        // CoInitializeEx returning S_FALSE means COM was already initialised here, which is
        // fine; only a hard failure is treated as unusable.
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr.is_err() {
                return None;
            }
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            Some(Self { automation })
        }
    }

}
