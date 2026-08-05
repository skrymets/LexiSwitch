//! Keyboard layout tables: what character a physical key position produces, and the
//! inverse.
//!
//! A table is built once from plain data and never mutated
//! ([ADR-0004](../../docs/adr/0004-runtime-derived-layout-tables.md)). Nothing here knows
//! where the data came from — at runtime `lexiswitch-win` probes `ToUnicodeEx`, and in tests
//! it is read from frozen JSON fixtures. That separation is why this crate needs no parser
//! and no operating system.

use std::collections::HashMap;

/// A physical key position: a set-1 scancode together with whether shift was held.
///
/// Shift is part of the position rather than a separate modifier because that is what makes
/// case fall out of conversion for free — the shifted position on one layout maps to the
/// shifted position on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyPos {
    pub scancode: u16,
    pub shift: bool,
}

impl KeyPos {
    pub const fn new(scancode: u16, shift: bool) -> Self {
        Self { scancode, shift }
    }
}

/// What a position produces under some layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValue {
    Char(char),
    /// A dead key — it produces no character on its own, only an accent pending on the next
    /// keystroke. Recorded as dead rather than resolved to a character, because it has no
    /// single character to resolve to.
    Dead,
}

/// `(scancode, shift) -> character` for one layout, with the inverse.
#[derive(Debug, Clone)]
pub struct LayoutTable {
    name: String,
    forward: HashMap<KeyPos, KeyValue>,
    reverse: HashMap<char, KeyPos>,
}

impl LayoutTable {
    /// Build a table from probed positions.
    ///
    /// Dead positions are recorded in the forward map but deliberately excluded from the
    /// inverse: a dead key produces no character, so no character can map back to it.
    ///
    /// Where two positions produce the same character, the first wins and later ones are
    /// ignored. This keeps construction total — there is no error case for a duplicate,
    /// because either position is a correct answer to "which key produces this character".
    pub fn new(
        name: impl Into<String>,
        entries: impl IntoIterator<Item = (KeyPos, KeyValue)>,
    ) -> Self {
        let mut forward = HashMap::new();
        let mut reverse: HashMap<char, KeyPos> = HashMap::new();

        for (pos, value) in entries {
            if let KeyValue::Char(c) = value {
                reverse.entry(c).or_insert(pos);
            }
            forward.insert(pos, value);
        }

        Self {
            name: name.into(),
            forward,
            reverse,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Number of positions in the table.
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// What this position produces, or `None` if the position was never probed.
    ///
    /// A miss and a dead key are different answers: `None` means "not part of this table",
    /// `Some(Dead)` means "a key that produces no character".
    pub fn value_at(&self, pos: KeyPos) -> Option<KeyValue> {
        self.forward.get(&pos).copied()
    }

    /// The character this position produces, or `None` if it is dead or unprobed.
    pub fn char_at(&self, pos: KeyPos) -> Option<char> {
        match self.value_at(pos) {
            Some(KeyValue::Char(c)) => Some(c),
            _ => None,
        }
    }

    pub fn is_dead(&self, pos: KeyPos) -> bool {
        matches!(self.value_at(pos), Some(KeyValue::Dead))
    }

    /// Which position produces this character, or `None` if the layout cannot produce it.
    pub fn position_of(&self, ch: char) -> Option<KeyPos> {
        self.reverse.get(&ch).copied()
    }

    /// Every position in the table, in no particular order.
    pub fn positions(&self) -> impl Iterator<Item = (KeyPos, KeyValue)> + '_ {
        self.forward.iter().map(|(pos, value)| (*pos, *value))
    }
}
