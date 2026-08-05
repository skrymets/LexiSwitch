//! What has keyboard focus, and whether it is a password field.
//!
//! This is the measurement the runtime privacy model rests on
//! ([ADR-0005](../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md)): the buffer must
//! not fill while a password field is focused. Detection is good, not perfect — reliable for
//! native Win32 and WinUI controls, weaker inside browsers and Electron applications — and
//! its real coverage is measured with `examples/probe_focus.rs` rather than assumed.
//!
//! **Cost matters here.** UI Automation calls can stall for a noticeable time, and this runs
//! on the message-loop thread, which must never block. The focused window handle is cheap to
//! read, so the expensive probe happens only when focus actually moves — see [`PasswordGate`].

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW, GetWindowThreadProcessId,
    GUITHREADINFO, GWL_STYLE,
};
use windows::Win32::Foundation::CloseHandle;
use windows::core::PWSTR;

/// `ES_PASSWORD` — the edit-control style that masks input.
const ES_PASSWORD: isize = 0x0020;

/// How a password verdict was reached, so coverage can be reported honestly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detection {
    /// UI Automation reported the `IsPassword` property.
    UiAutomation,
    /// UI Automation was unavailable; the window's `ES_PASSWORD` style answered instead.
    WindowStyle,
    /// Neither could answer. Treated as *not* a password field, which is the permissive
    /// direction — the exclusion list and the pause control are the backstops.
    Unknown,
}

/// What currently has keyboard focus.
#[derive(Debug, Clone)]
pub struct FocusInfo {
    /// Executable name of the process owning the focused window, e.g. `KeePassXC.exe`.
    pub process: String,
    pub is_password: bool,
    pub detection: Detection,
    /// The focused window, or the foreground window where focus could not be determined.
    pub hwnd: HWND,
}

/// The focused window, falling back to the foreground window.
///
/// These differ more often than one might expect: Windows 11's Notepad puts its text area on
/// a different thread from its frame window, and packaged applications interpose a host
/// process entirely.
fn focused_window() -> HWND {
    // SAFETY: GUITHREADINFO carries its own size as the API requires; thread id 0 asks about
    // the foreground thread. Both calls tolerate there being no such window.
    unsafe {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(0, &mut info).is_ok() && !info.hwndFocus.is_invalid() {
            info.hwndFocus
        } else {
            GetForegroundWindow()
        }
    }
}

/// Executable name of the process owning `hwnd`.
///
/// Returns an empty string when the process cannot be opened — which happens for elevated
/// processes seen from an unelevated one, and is itself informative.
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

    /// Whether the focused element reports itself as a password field.
    fn is_password(&self) -> Option<bool> {
        // SAFETY: `automation` is a live interface owned by this type. GetFocusedElement
        // returns a new reference that windows-rs releases on drop.
        unsafe {
            let element = self.automation.GetFocusedElement().ok()?;
            element.CurrentIsPassword().ok().map(|b| b.as_bool())
        }
    }
}

/// Inspect what has focus now.
///
/// `uia` is optional so callers without UI Automation still get the window-style answer.
pub fn inspect(uia: Option<&Uia>) -> FocusInfo {
    let hwnd = focused_window();
    let process = process_name(hwnd);

    if let Some(uia) = uia {
        if let Some(is_password) = uia.is_password() {
            return FocusInfo {
                process,
                is_password,
                detection: Detection::UiAutomation,
                hwnd,
            };
        }
    }

    // Fallback: the classic edit-control style. Only meaningful for real Win32 controls, so
    // an answer of "no" here is weaker than an answer of "no" from automation.
    if !hwnd.is_invalid() {
        // SAFETY: hwnd is a window handle obtained above; GetWindowLongPtrW on a valid
        // window returns its style bits, and 0 for an invalid one.
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
        if style != 0 {
            return FocusInfo {
                process,
                is_password: style & ES_PASSWORD != 0,
                detection: Detection::WindowStyle,
                hwnd,
            };
        }
    }

    FocusInfo {
        process,
        is_password: false,
        detection: Detection::Unknown,
        hwnd,
    }
}

/// Decides whether keystrokes may be recorded, without paying for a probe on every key.
///
/// Reading the focused window handle is cheap; asking UI Automation about it is not, and
/// this runs on the thread that must never block. The verdict is therefore cached per
/// window and recomputed only when focus moves.
pub struct PasswordGate {
    uia: Option<Uia>,
    cached_for: Option<HWND>,
    cached: Option<FocusInfo>,
}

impl PasswordGate {
    pub fn new() -> Self {
        Self {
            uia: Uia::new(),
            cached_for: None,
            cached: None,
        }
    }

    /// Whether UI Automation is available at all.
    pub fn has_automation(&self) -> bool {
        self.uia.is_some()
    }

    /// The automation interface, for callers that need it directly — reading a selection,
    /// for instance. Shared rather than re-created, because `CoCreateInstance` is not cheap
    /// and this runs on the thread that must not block.
    pub fn automation(&self) -> Option<&IUIAutomation> {
        self.uia.as_ref().map(|u| &u.automation)
    }

    /// Current focus, probing only if the focused window changed since last time.
    pub fn current(&mut self) -> &FocusInfo {
        let hwnd = focused_window();
        if self.cached_for != Some(hwnd) || self.cached.is_none() {
            self.cached = Some(inspect(self.uia.as_ref()));
            self.cached_for = Some(hwnd);
        }
        self.cached.as_ref().expect("just populated")
    }

    /// Force the next query to re-probe — after a focus change is known to have happened.
    pub fn invalidate(&mut self) {
        self.cached_for = None;
    }
}

impl Default for PasswordGate {
    fn default() -> Self {
        Self::new()
    }
}
