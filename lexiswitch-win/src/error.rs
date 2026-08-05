//! Errors from Win32 calls, carrying enough to say what failed rather than only that
//! something did.
//!
//! The `agent-control` spec requires notifications that state what did not happen — "hotkey
//! Ctrl+Pause is already in use", not "error 5" — so the call name travels with the code.

use std::fmt;

/// A failed Win32 call.
#[derive(Debug, Clone)]
pub struct WinError {
    call: &'static str,
    code: windows::core::HRESULT,
    message: String,
}

impl WinError {
    pub fn new(call: &'static str, source: windows::core::Error) -> Self {
        Self {
            call,
            code: source.code(),
            message: source.message(),
        }
    }

    /// Build from the thread's last error, for APIs that signal failure by return value
    /// rather than by returning a `Result`.
    pub fn last(call: &'static str) -> Self {
        Self::new(call, windows::core::Error::from_thread())
    }

    /// The API that failed.
    pub fn call(&self) -> &'static str {
        self.call
    }

    pub fn code(&self) -> windows::core::HRESULT {
        self.code
    }

    /// The system's description, already fit to show a user.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for WinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {} ({:#010x})", self.call, self.message, self.code.0)
    }
}

impl std::error::Error for WinError {}
