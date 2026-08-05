//! Converting text between layouts by physical key position.
//!
//! Each character is resolved to the key that produces it under the source layout, then to
//! whatever that key produces under the target. Case comes along for free, because shift is
//! part of the position.

use crate::layout::LayoutTable;

/// Convert `text` from one layout to another.
///
/// Returns `None` when the result would be identical to the input — see
/// [`convert_lossy`] for the distinction and why it is drawn here rather than left to the
/// caller.
///
/// Characters the source layout cannot produce, and positions that are dead in either
/// layout, pass through unchanged rather than being dropped or substituted.
pub fn convert(text: &str, from: &LayoutTable, to: &LayoutTable) -> Option<String> {
    let converted = convert_lossy(text, from, to);
    (converted != text).then_some(converted)
}

/// Convert, always returning the result even when nothing changed.
///
/// Useful when the caller has already decided to act and only wants the text. Prefer
/// [`convert`] where "nothing would change" is a meaningful answer — usually which, for
/// a hotkey that should stay silent rather than rewrite a word with itself, it is.
pub fn convert_lossy(text: &str, from: &LayoutTable, to: &LayoutTable) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        out.push(convert_char(ch, from, to).unwrap_or(ch));
    }
    out
}

/// Convert one character, or `None` if it cannot be mapped.
///
/// Unmappable means one of three things, all of which are pass-through rather than error:
/// the source layout does not produce this character at all; the position it occupies was
/// never probed on the target; or that position is a dead key on the target, which produces
/// no character to substitute.
fn convert_char(ch: char, from: &LayoutTable, to: &LayoutTable) -> Option<char> {
    let pos = from.position_of(ch)?;
    to.char_at(pos)
}
