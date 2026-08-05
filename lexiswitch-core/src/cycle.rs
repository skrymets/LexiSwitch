//! Multi-tap cycling: choosing a target layout, disambiguating, and undoing — all one rule.
//!
//! Per [ADR-0009](../../docs/adr/0009-multi-tap-layout-cycling.md), the nth press within the
//! multi-tap window renders the retained source text under the nth candidate layout, and
//! after the last candidate the cycle returns to the original. There is no conversion
//! history and no undo state machine, because the general rule already produces undo: with
//! two layouts installed, the second press *is* the original.
//!
//! Everything here is a pure function of source text, candidate list, and press count, which
//! is why it lives in this crate and needs no Win32 to test.

use crate::convert::convert_lossy;
use crate::layout::LayoutTable;

/// Render `source` for the `press`-th tap of a multi-tap run.
///
/// Press 0 and the wrap position both give the original text back. Every press renders from
/// `source` rather than from the previous result, so a press is idempotent for its count and
/// repeated taps cannot drift by converting already-converted text.
pub fn render(source: &str, from: &LayoutTable, candidates: &[&LayoutTable], press: usize) -> String {
    if candidates.is_empty() || press == 0 {
        return source.to_string();
    }
    // The cycle has one more stop than there are candidates: the last is the original.
    let stops = candidates.len() + 1;
    let index = (press - 1) % stops;
    match candidates.get(index) {
        Some(target) => convert_lossy(source, from, target),
        // Past the last candidate — wrap home.
        None => source.to_string(),
    }
}

/// Classifies presses into runs by their message timestamps.
///
/// Timestamps come from the message that carried the keystroke, never from a clock read
/// inside the handler: a handler read measures when the process got around to looking, so
/// scheduling delay in the message loop would corrupt the count.
#[derive(Debug, Clone)]
pub struct MultiTap {
    window_ms: u32,
    last_tick: Option<u32>,
    press: usize,
}

impl MultiTap {
    /// The spike measured deliberate multi-taps at 141–157 ms and deliberate separations
    /// above 2500 ms, so 500 ms sits with wide margin on both sides.
    pub fn new(window_ms: u32) -> Self {
        Self {
            window_ms,
            last_tick: None,
            press: 0,
        }
    }

    /// Register a press and return its position in the current run, counting from 1.
    pub fn press(&mut self, tick: u32) -> usize {
        let continues = match self.last_tick {
            // `wrapping_sub` so the ~49-day rollover of a 32-bit millisecond counter
            // cannot make an old press look adjacent to a new one.
            Some(last) => tick.wrapping_sub(last) <= self.window_ms,
            None => false,
        };
        self.press = if continues { self.press + 1 } else { 1 };
        self.last_tick = Some(tick);
        self.press
    }

    /// End the current run. The next press starts a new correction.
    pub fn reset(&mut self) {
        self.last_tick = None;
        self.press = 0;
    }

    /// Position in the current run, or 0 if no run is in progress.
    pub fn current(&self) -> usize {
        self.press
    }
}
