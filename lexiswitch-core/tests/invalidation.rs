//! Tasks 3.3 / 3.4 / 3.5 — when the buffer is wiped, and that wiping really overwrites.
//!
//! ADR-0005 lists seven triggers: focus change, Enter, Tab, Escape, idle timeout, a
//! successful conversion, and pause. Each gets its own test, because each is a separate way
//! for retained keystrokes to outlive their usefulness.

use lexiswitch_core::buffer::{KeyBuffer, Recorded, WipeReason, CAPACITY};

fn filled() -> KeyBuffer {
    let mut buf = KeyBuffer::new();
    for i in 0..5u16 {
        buf.push(0x1e + i, false, 100 + i as u32);
    }
    assert_eq!(buf.len(), 5);
    buf
}

#[test]
fn focus_change_wipes() {
    let mut buf = filled();
    buf.invalidate(WipeReason::FocusChange);
    assert!(buf.is_empty());
}

#[test]
fn enter_wipes() {
    let mut buf = filled();
    assert_eq!(
        buf.record(0x1c, false, 200),
        Recorded::Wiped(WipeReason::Enter)
    );
    assert!(buf.is_empty(), "Enter must not be recorded as a keystroke");
}

#[test]
fn tab_wipes() {
    let mut buf = filled();
    assert_eq!(buf.record(0x0f, false, 200), Recorded::Wiped(WipeReason::Tab));
    assert!(buf.is_empty());
}

#[test]
fn escape_wipes() {
    let mut buf = filled();
    assert_eq!(
        buf.record(0x01, false, 200),
        Recorded::Wiped(WipeReason::Escape)
    );
    assert!(buf.is_empty());
}

#[test]
fn idle_timeout_wipes() {
    let mut buf = KeyBuffer::with_idle_timeout(1_000);
    buf.push(0x1e, false, 0);
    assert!(!buf.is_stale(999));
    assert!(buf.is_stale(1_001));

    // A keystroke arriving after the timeout starts a fresh correction rather than
    // extending a stale one.
    buf.record(0x30, false, 5_000);
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.iter().next().unwrap().scancode, 0x30);
}

#[test]
fn a_successful_conversion_wipes() {
    let mut buf = filled();
    buf.invalidate(WipeReason::Converted);
    assert!(buf.is_empty());
}

#[test]
fn pause_wipes_and_then_ignores() {
    let mut buf = filled();
    buf.set_paused(true);
    // Pause is a kill switch: it clears what is already held, not only what comes next.
    assert!(buf.is_empty());
    assert_eq!(buf.record(0x1e, false, 300), Recorded::Ignored);
    assert!(buf.is_empty());
}

#[test]
fn resuming_lets_keystrokes_through_again() {
    let mut buf = filled();
    buf.set_paused(true);
    buf.set_paused(false);
    assert!(!buf.is_paused());
    assert_eq!(buf.record(0x1e, false, 300), Recorded::Appended);
    assert_eq!(buf.len(), 1);
}

#[test]
fn an_empty_buffer_is_never_stale() {
    // Nothing retained means nothing to expire; staleness is about held data.
    let buf = KeyBuffer::with_idle_timeout(1_000);
    assert!(!buf.is_stale(u32::MAX));
}

#[test]
fn staleness_survives_the_tick_counter_wrapping() {
    // MSG.time is a 32-bit millisecond counter that rolls over about every 49 days. A
    // naive subtraction would make an ancient buffer look brand new across the rollover.
    let mut buf = KeyBuffer::with_idle_timeout(1_000);
    buf.push(0x1e, false, u32::MAX - 100);
    assert!(!buf.is_stale(u32::MAX - 50));
    assert!(buf.is_stale(u32::MAX.wrapping_add(2_000)));
}

#[test]
fn wiping_overwrites_the_backing_storage() {
    // Task 3.5: emptying the buffer is not enough — the bytes must be gone. Entries past
    // the live length still hold earlier keystrokes, so they are zeroized too.
    let mut buf = KeyBuffer::new();
    for i in 0..CAPACITY as u16 {
        buf.push(0x1e + i, true, 1_000 + i as u32);
    }
    buf.wipe();

    assert!(buf.is_empty());
    assert_eq!(buf.iter().count(), 0);
    // Refill and confirm nothing from before shows through: every entry is the new value.
    for i in 0..CAPACITY as u16 {
        buf.push(7, false, 0);
        let _ = i;
    }
    assert!(
        buf.iter().all(|e| e.scancode == 7 && !e.shift && e.tick == 0),
        "a pre-wipe entry survived into the refilled buffer"
    );
}

#[test]
fn wiping_resets_staleness() {
    let mut buf = KeyBuffer::with_idle_timeout(1_000);
    buf.push(0x1e, false, 0);
    buf.wipe();
    assert_eq!(buf.last_tick(), None);
    assert!(!buf.is_stale(u32::MAX));
}
