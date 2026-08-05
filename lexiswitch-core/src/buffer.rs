//! The bounded keystroke buffer.
//!
//! This is the one component that holds data passively, between explicit user actions, so
//! its containment rules *are* the runtime privacy model
//! ([ADR-0005](../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md)). The cap, the
//! wipe triggers, and the zeroizing are security properties rather than housekeeping.
//!
//! What it holds is physical key positions, never characters. Resolving positions to text
//! needs a layout table and happens only at extraction time, so a memory disclosure yields
//! scancodes that need a layout to interpret. A modest gain, but free.

use zeroize::Zeroize;

use crate::layout::{KeyPos, LayoutTable};

/// Maximum retained keystrokes. Bounding exposure is the point; this is not a tuning knob.
pub const CAPACITY: usize = 64;

/// Scancodes that end a correction rather than contributing to one.
const SC_ESCAPE: u16 = 0x01;
const SC_TAB: u16 = 0x0f;
const SC_ENTER: u16 = 0x1c;
/// Removes the previous keystroke rather than joining the word.
const SC_BACKSPACE: u16 = 0x0e;
/// Ends a word without ending the correction.
const SC_SPACE: u16 = 0x39;

/// One observed keystroke, as a physical position plus when it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Zeroize)]
pub struct KeyEntry {
    pub scancode: u16,
    pub shift: bool,
    /// Milliseconds, from the message that carried the keystroke. Wrapping is handled by
    /// comparing with `wrapping_sub`, so a ~49-day rollover cannot make the buffer look
    /// arbitrarily fresh.
    pub tick: u32,
}

/// Why the buffer was cleared. Recorded for the caller's benefit; the buffer treats every
/// reason identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeReason {
    FocusChange,
    Enter,
    Tab,
    Escape,
    IdleTimeout,
    Converted,
    Paused,
}

/// What happened to a keystroke offered to the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// Appended, possibly evicting the oldest entry.
    Appended,
    /// The keystroke ended the correction instead of joining it.
    Wiped(WipeReason),
    /// The buffer was not accepting input.
    Ignored,
}

/// A fixed 64-entry ring of recent keystrokes.
///
/// The storage is a fixed array rather than a growable buffer so that no reallocation can
/// leave a copy of earlier keystrokes elsewhere in the heap for the allocator to hand out
/// later. Wiping overwrites in place.
#[derive(Debug)]
pub struct KeyBuffer {
    entries: [KeyEntry; CAPACITY],
    /// Index of the oldest entry.
    head: usize,
    len: usize,
    /// How long the buffer may sit untouched before it is considered stale, in ms.
    idle_timeout_ms: u32,
    /// While paused, keystrokes are ignored entirely rather than recorded and hidden.
    paused: bool,
}

impl Default for KeyBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyBuffer {
    /// A buffer with the default 30-second idle timeout.
    pub fn new() -> Self {
        Self::with_idle_timeout(30_000)
    }

    pub fn with_idle_timeout(idle_timeout_ms: u32) -> Self {
        Self {
            entries: [KeyEntry::default(); CAPACITY],
            head: 0,
            len: 0,
            idle_timeout_ms,
            paused: false,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Append a keystroke unconditionally, evicting the oldest entry when full.
    ///
    /// Prefer [`record`](Self::record), which applies the invalidation rules. This exists
    /// for callers that have already decided the keystroke belongs in the buffer.
    pub fn push(&mut self, scancode: u16, shift: bool, tick: u32) {
        let slot = (self.head + self.len) % CAPACITY;
        self.entries[slot] = KeyEntry {
            scancode,
            shift,
            tick,
        };
        if self.len == CAPACITY {
            // Full: the write above overwrote the oldest entry, so the window slides.
            self.head = (self.head + 1) % CAPACITY;
        } else {
            self.len += 1;
        }
    }

    /// Oldest to newest. Reversible, because extraction walks backwards from the most
    /// recent keystroke.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &KeyEntry> + '_ {
        (0..self.len).map(move |i| &self.entries[(self.head + i) % CAPACITY])
    }

    /// Timestamp of the most recent keystroke.
    pub fn last_tick(&self) -> Option<u32> {
        (self.len > 0).then(|| self.entries[(self.head + self.len - 1) % CAPACITY].tick)
    }

    /// Whether the buffer has sat untouched past its idle timeout.
    pub fn is_stale(&self, now: u32) -> bool {
        match self.last_tick() {
            None => false,
            Some(last) => now.wrapping_sub(last) > self.idle_timeout_ms,
        }
    }

    /// Address and byte length of the backing storage.
    ///
    /// Exists so the platform layer can page-lock these bytes, keeping recent keystrokes out
    /// of the pagefile (ADR-0005). This crate forbids `unsafe` and cannot make that call
    /// itself; handing out the range is safe, and only using it is not.
    ///
    /// The address is only stable while the buffer is not moved, so a caller that locks it
    /// must keep the buffer at a fixed address — box it, or own it for the process lifetime.
    pub fn backing(&self) -> (*const u8, usize) {
        (
            self.entries.as_ptr() as *const u8,
            std::mem::size_of_val(&self.entries),
        )
    }

    /// Clear the buffer, overwriting the backing storage.
    ///
    /// Every entry is zeroized, not just the live ones — entries beyond `len` still hold
    /// keystrokes from before the last wipe, and leaving them would defeat the point.
    pub fn wipe(&mut self) {
        self.entries.zeroize();
        self.head = 0;
        self.len = 0;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            // Pause is a kill switch, so it takes effect on what is already held, not only
            // on what arrives next.
            self.wipe();
        }
    }

    /// Offer a keystroke to the buffer, applying the invalidation rules.
    pub fn record(&mut self, scancode: u16, shift: bool, tick: u32) -> Recorded {
        if self.paused {
            return Recorded::Ignored;
        }
        if let Some(reason) = match scancode {
            SC_ENTER => Some(WipeReason::Enter),
            SC_TAB => Some(WipeReason::Tab),
            SC_ESCAPE => Some(WipeReason::Escape),
            _ => None,
        } {
            self.wipe();
            return Recorded::Wiped(reason);
        }
        // A keystroke arriving after the timeout starts a new correction rather than
        // extending a stale one.
        if self.is_stale(tick) {
            self.wipe();
        }
        if scancode == SC_BACKSPACE {
            // Beyond ADR-0005's listed rules, and load-bearing: the extracted word length
            // decides how many backspaces get injected into the user's document. Leaving a
            // corrected typo in the buffer would overcount and delete text they did type.
            self.pop();
            return Recorded::Appended;
        }
        self.push(scancode, shift, tick);
        Recorded::Appended
    }

    /// Drop the most recent entry, zeroizing the slot it occupied.
    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }
        self.len -= 1;
        let slot = (self.head + self.len) % CAPACITY;
        self.entries[slot].zeroize();
    }

    /// The word most recently typed, resolved through `layout`.
    ///
    /// `None` when there is nothing usable: an empty buffer, contents older than the idle
    /// timeout, or a trailing space meaning no word is in progress. Declining is required
    /// rather than merely tidy — the `manual-correction` spec calls for a no-op when the
    /// buffer is stale, never a guess.
    ///
    /// A word ends at a space, at a position this layout cannot resolve, or at the oldest
    /// retained entry. That last case means an unusually long word comes back truncated to
    /// what the 64-entry cap held, which is the cap doing its job.
    pub fn last_word(&self, layout: &LayoutTable, now: u32) -> Option<String> {
        if self.is_empty() || self.is_stale(now) {
            return None;
        }

        let mut chars: Vec<char> = Vec::new();
        for entry in self.iter().rev() {
            if entry.scancode == SC_SPACE {
                break;
            }
            match layout.char_at(KeyPos::new(entry.scancode, entry.shift)) {
                Some(c) => chars.push(c),
                // Unresolvable: a dead key, or a position never probed. Either way it
                // cannot be part of a word.
                None => break,
            }
        }

        if chars.is_empty() {
            return None;
        }
        chars.reverse();
        Some(chars.into_iter().collect())
    }

    /// Clear for a reason that is not a keystroke.
    pub fn invalidate(&mut self, reason: WipeReason) {
        let _ = reason;
        self.wipe();
    }
}

impl Drop for KeyBuffer {
    fn drop(&mut self) {
        self.entries.zeroize();
    }
}
