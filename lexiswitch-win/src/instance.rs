//! One agent per user session.
//!
//! A second LexiSwitch shares everything and cooperates on none of it: the same configuration
//! file, either of them writing it; the same log; a second identical tray icon; and a shortcut
//! it cannot register because the first already holds it, so it comes up reporting a conflict
//! with itself and offering to fix it by changing the shortcut — which is not the problem.
//!
//! ## Why a mutex rather than looking for the window
//!
//! Two shortcuts clicked in quick succession is an ordinary thing for a user to do. Searching
//! for the existing window is a check-then-act: both instances can look before either has
//! created its window, and both proceed. `CreateMutexW` decides in the kernel, so exactly one
//! side sees `ERROR_ALREADY_EXISTS` however close the race.
//!
//! It also cannot leave anything behind. The mutex is released when the process ends, whatever
//! ends it, so a crash cannot lock the user out of their own program — which a lock file could.
//!
//! ## Why the name is session-local
//!
//! No `Global\` prefix, so the name lives in the session's namespace. Two users signed in to
//! one machine are two users and get one agent each. A global name would let whoever signed in
//! first deny everyone else an agent entirely.

use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, HANDLE,
};
use windows::Win32::System::Threading::CreateMutexW;

/// Held for the lifetime of the process that won.
///
/// Dropping it releases the name, so it is kept alive deliberately rather than discarded.
pub struct InstanceGuard {
    /// `None` when the mutex could not be created at all — see the fallback in [`acquire`].
    /// The guard still exists so the caller has one type to hold either way.
    handle: Option<HANDLE>,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let Some(handle) = self.handle else {
            return;
        };
        // SAFETY: the handle came from CreateMutexW in `acquire`, is owned solely by this
        // guard, and is closed exactly once because `handle` is taken by value on drop.
        // Windows would release it at process exit anyway; this covers a guard dropped by a
        // program that keeps running.
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
}

/// Whether this process is the one that runs.
pub enum Instance {
    /// Nothing else holds the name. The guard must be kept for as long as the program runs.
    First(InstanceGuard),
    /// Another instance is already running, and this one should not start.
    AlreadyRunning,
}

/// Claim the right to be the running instance.
pub fn acquire() -> Instance {
    // SAFETY: the name is a static NUL-terminated wide string. A null security attributes
    // pointer asks for the default descriptor, and not requesting ownership is what makes this
    // a name reservation rather than a lock anyone waits on.
    let handle = unsafe { CreateMutexW(None, false, w!("Local\\LexiSwitch.SingleInstance")) };

    match handle {
        Ok(handle) => {
            // Documented pattern: the call succeeds either way, and the last error is what
            // says whether this process created the name or merely opened an existing one.
            //
            // SAFETY: no preconditions; reports the error from the call above on this thread.
            let last = unsafe { GetLastError() };
            if last == ERROR_ALREADY_EXISTS {
                // A valid handle to somebody else's mutex. Closed immediately: holding it would
                // keep the name alive after the first instance exits, and this process is about
                // to leave anyway.
                //
                // SAFETY: the handle was just returned by CreateMutexW and is not used again.
                unsafe {
                    let _ = CloseHandle(handle);
                }
                Instance::AlreadyRunning
            } else {
                Instance::First(InstanceGuard {
                    handle: Some(handle),
                })
            }
        }
        // Access denied means something already owns the name with a descriptor this process
        // cannot open — an elevated instance, most likely. Treating that as "free to start"
        // would produce exactly the two-agent situation this exists to prevent, so it counts as
        // already running. It is wrong only if some unrelated object holds the name, and
        // declining is the safer way to be wrong.
        Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => Instance::AlreadyRunning,
        // Anything else means the guard could not be established at all. Starting is the better
        // failure: refusing to run because a mutex could not be created would make an
        // unexplained system condition look like the program being broken.
        Err(_) => Instance::First(InstanceGuard { handle: None }),
    }
}
