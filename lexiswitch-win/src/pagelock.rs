//! Keeping the keystroke buffer out of the pagefile.
//!
//! `VirtualLock` pins pages in physical memory, so recent keystrokes cannot be written to
//! disk by the memory manager. That closes one route by which something derived from typing
//! could reach persistent storage (ADR-0005) — it does not close hibernation or a crash
//! dump, which are handled by not producing dumps at all.
//!
//! This is best-effort by design: the per-process working-set limit can refuse the lock, and
//! a refusal must not stop the program. It is reported so the reduction in guarantee is
//! visible rather than silent.

use std::ops::{Deref, DerefMut};

use lexiswitch_core::buffer::KeyBuffer;
use windows::Win32::System::Memory::{VirtualLock, VirtualUnlock};

use crate::WinError;

/// A keystroke buffer whose pages are locked into physical memory.
///
/// Owning the buffer is what makes this safe to expose: a `Box` keeps its contents at a
/// fixed address for as long as it lives, so the locked range cannot be invalidated by the
/// buffer moving. Handing out `&mut` is still fine — mutating the contents does not relocate
/// them.
///
/// Construction never fails. If the lock is refused the buffer still works and the error is
/// returned alongside it, because losing the pagefile guarantee is worth reporting but not
/// worth refusing to run over.
pub struct LockedBuffer {
    // Declared before `_lock` so the buffer is dropped last: the lock's Drop still refers to
    // this memory.
    buffer: Box<KeyBuffer>,
    lock: Option<PageLock>,
}

impl LockedBuffer {
    pub fn new(buffer: KeyBuffer) -> (Self, Option<WinError>) {
        let buffer = Box::new(buffer);
        let (addr, len) = buffer.backing();
        // SAFETY: `buffer` is boxed and owned by the value being constructed, so the range
        // is live and at a fixed address for as long as the lock exists.
        match unsafe { PageLock::new(addr, len) } {
            Ok(lock) => (
                Self {
                    buffer,
                    lock: Some(lock),
                },
                None,
            ),
            Err(e) => (Self { buffer, lock: None }, Some(e)),
        }
    }

    /// Whether the pages are actually locked, as opposed to merely requested.
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
    }
}

impl Deref for LockedBuffer {
    type Target = KeyBuffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for LockedBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

/// A page-locked region, unlocked when dropped.
pub struct PageLock {
    addr: *const u8,
    len: usize,
}

impl PageLock {
    /// Lock `len` bytes at `addr` into physical memory.
    ///
    /// # Safety
    ///
    /// `addr` must point to `len` bytes of live memory that stays at this address for the
    /// lifetime of the returned `PageLock`. Locking pages that are later freed or moved
    /// leaves the lock applied to memory this process no longer owns.
    pub unsafe fn new(addr: *const u8, len: usize) -> Result<Self, WinError> {
        // SAFETY: the caller guarantees the range is live and fixed for our lifetime, which
        // is exactly what VirtualLock requires.
        unsafe {
            VirtualLock(addr as *mut _, len).map_err(|e| WinError::new("VirtualLock", e))?;
        }
        Ok(Self { addr, len })
    }
}

impl Drop for PageLock {
    fn drop(&mut self) {
        // SAFETY: unlocks exactly the range this type locked and has not unlocked before.
        // A failure here cannot be acted on and is deliberately ignored.
        unsafe {
            let _ = VirtualUnlock(self.addr as *mut _, self.len);
        }
    }
}

// SAFETY: a `PageLock` holds an address it neither owns nor ever dereferences — only passes
// back to `VirtualUnlock`. Moving that between threads cannot create a data race. `Sync` is
// deliberately not implemented: unlocking is not idempotent, so two threads dropping the
// same lock concurrently would unlock twice.
unsafe impl Send for PageLock {}
