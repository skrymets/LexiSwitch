//! Task 3.1 — the bounded ring.
//!
//! The cap is the privacy model, not an optimisation ([ADR-0005](../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md)):
//! exposure is bounded by refusing to hold more than 64 keystrokes, so "capacity is never
//! exceeded" is a security property and is tested as one.

use lexiswitch_core::buffer::{KeyBuffer, CAPACITY};

#[test]
fn a_new_buffer_is_empty() {
    let buf = KeyBuffer::new();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
    assert_eq!(CAPACITY, 64);
}

#[test]
fn entries_are_kept_in_the_order_they_were_typed() {
    let mut buf = KeyBuffer::new();
    for (i, sc) in [0x1e_u16, 0x30, 0x2e].iter().enumerate() {
        buf.push(*sc, false, i as u32);
    }
    let got: Vec<u16> = buf.iter().map(|e| e.scancode).collect();
    assert_eq!(got, vec![0x1e, 0x30, 0x2e]);
}

#[test]
fn shift_state_and_tick_are_retained_per_entry() {
    let mut buf = KeyBuffer::new();
    buf.push(0x1e, true, 500);
    let e = buf.iter().next().expect("one entry");
    assert_eq!(e.scancode, 0x1e);
    assert!(e.shift);
    assert_eq!(e.tick, 500);
}

#[test]
fn capacity_is_never_exceeded() {
    let mut buf = KeyBuffer::new();
    for i in 0..(CAPACITY * 3) {
        buf.push(i as u16, false, i as u32);
        assert!(
            buf.len() <= CAPACITY,
            "len {} exceeded capacity after {} pushes",
            buf.len(),
            i + 1
        );
    }
    assert_eq!(buf.len(), CAPACITY);
}

#[test]
fn the_oldest_entry_is_discarded_when_full() {
    let mut buf = KeyBuffer::new();
    // Fill exactly, then push one more.
    for i in 0..CAPACITY {
        buf.push(i as u16, false, i as u32);
    }
    assert_eq!(buf.iter().next().unwrap().scancode, 0);

    buf.push(999, false, 999);
    assert_eq!(buf.len(), CAPACITY);
    // 0 is gone; the window has slid forward by one.
    assert_eq!(buf.iter().next().unwrap().scancode, 1);
    assert_eq!(buf.iter().last().unwrap().scancode, 999);
}

#[test]
fn a_full_buffer_keeps_exactly_the_most_recent_entries() {
    let mut buf = KeyBuffer::new();
    for i in 0..(CAPACITY as u16 + 10) {
        buf.push(i, false, i as u32);
    }
    let got: Vec<u16> = buf.iter().map(|e| e.scancode).collect();
    let expected: Vec<u16> = (10..(CAPACITY as u16 + 10)).collect();
    assert_eq!(got, expected);
}

#[test]
fn wiping_empties_the_buffer() {
    let mut buf = KeyBuffer::new();
    for i in 0..10 {
        buf.push(i, false, i as u32);
    }
    buf.wipe();
    assert!(buf.is_empty());
    assert_eq!(buf.iter().count(), 0);
}
