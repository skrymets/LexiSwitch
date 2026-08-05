//! Tasks 3.6 / 3.7 — pulling the last typed word out of the buffer.
//!
//! Resolution happens here, not at capture time. The buffer holds positions; deciding where
//! a word begins needs those positions interpreted as characters, which needs a layout
//! table. Keeping it late also keeps the capture path trivial, which matters because that
//! path runs on the message-loop thread.

mod common;

use lexiswitch_core::buffer::KeyBuffer;

// en-US scancodes for the letters used below.
const T: u16 = 0x14;
const E: u16 = 0x12;
const S: u16 = 0x1f;
const A: u16 = 0x1e;
const B: u16 = 0x30;
const SPACE: u16 = 0x39;
const BACKSPACE: u16 = 0x0e;

fn type_keys(buf: &mut KeyBuffer, keys: &[(u16, bool)], start_tick: u32) {
    for (i, (sc, shift)) in keys.iter().enumerate() {
        buf.record(*sc, *shift, start_tick + i as u32);
    }
}

#[test]
fn an_empty_buffer_has_no_word() {
    let en = common::load("en_us");
    let buf = KeyBuffer::new();
    assert_eq!(buf.last_word(&en, 0), None);
}

#[test]
fn a_stale_buffer_has_no_word() {
    // Declining to act on stale contents is required: the manual-correction spec says an
    // unusable buffer is a no-op, never a guess.
    let en = common::load("en_us");
    let mut buf = KeyBuffer::with_idle_timeout(1_000);
    type_keys(&mut buf, &[(T, false), (E, false), (S, false), (T, false)], 0);

    assert_eq!(buf.last_word(&en, 500).as_deref(), Some("test"));
    assert_eq!(buf.last_word(&en, 100_000), None);
}

#[test]
fn the_last_word_is_resolved_through_the_given_layout() {
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, false), (E, false), (S, false), (T, false)], 0);
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("test"));
}

#[test]
fn the_same_keystrokes_resolve_differently_under_a_different_layout() {
    // The point of storing positions rather than characters: one buffer, two readings.
    let en = common::load("en_us");
    let uk = common::load("uk");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, false), (E, false), (S, false), (T, false)], 0);

    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("test"));
    assert_eq!(buf.last_word(&uk, 10).as_deref(), Some("еуіе"));
}

#[test]
fn only_the_trailing_word_is_extracted() {
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(
        &mut buf,
        &[(T, false), (E, false), (SPACE, false), (A, false), (B, false)],
        0,
    );
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("ab"));
}

#[test]
fn a_trailing_space_means_there_is_no_word_to_correct() {
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, false), (E, false), (SPACE, false)], 0);
    assert_eq!(buf.last_word(&en, 10), None);
}

#[test]
fn case_is_carried_through_extraction() {
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, true), (E, false), (S, false), (T, false)], 0);
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("Test"));
}

#[test]
fn backspace_removes_the_previous_keystroke() {
    // Beyond the literal task, and deliberately: the word length drives how many
    // backspaces get injected into someone's document. A miscount deletes text the user
    // did not type, which the design calls out as the destructive failure mode.
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(
        &mut buf,
        &[(T, false), (E, false), (S, false), (B, false), (BACKSPACE, false), (T, false)],
        0,
    );
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("test"));
}

#[test]
fn backspace_on_an_empty_buffer_is_harmless() {
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(BACKSPACE, false), (BACKSPACE, false)], 0);
    assert!(buf.is_empty());
    assert_eq!(buf.last_word(&en, 10), None);
}

#[test]
fn a_position_the_layout_cannot_resolve_ends_the_word() {
    // Function keys and the like were never probed, so they cannot be part of a word.
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, false), (0x3b, false), (A, false), (B, false)], 0);
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("ab"));
}

#[test]
fn extraction_does_not_consume_the_buffer() {
    // Multi-tap re-renders from the same source repeatedly, so extracting must be
    // repeatable and stable.
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    type_keys(&mut buf, &[(T, false), (E, false), (S, false), (T, false)], 0);
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("test"));
    assert_eq!(buf.last_word(&en, 10).as_deref(), Some("test"));
    assert_eq!(buf.len(), 4);
}

#[test]
fn a_word_longer_than_the_buffer_is_truncated_to_what_was_retained() {
    // 64 entries is a privacy bound, so an unusually long word is clipped rather than the
    // cap being raised. The extraction reports what is held, not what was typed.
    let en = common::load("en_us");
    let mut buf = KeyBuffer::new();
    for i in 0..100 {
        buf.record(A, false, i);
    }
    let word = buf.last_word(&en, 200).expect("a word");
    assert_eq!(word.len(), 64);
    assert!(word.chars().all(|c| c == 'a'));
}
