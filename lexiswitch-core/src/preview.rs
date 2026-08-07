//! Fitting a candidate into a row without hiding the thing being decided.
//!
//! A chooser row is a fixed width; a selection is not. Truncating at the width would be
//! correct and useless: two candidates that differ only near the end of a long selection would
//! render as identical rows, and the user would be asked to choose between them with the
//! evidence off screen.
//!
//! So the window is placed over the region that *differs* from the original, and any content
//! outside it is reported rather than silently dropped. The spec requires both — omissions
//! visible, changed text favoured — and leaves the algorithm open
//! (`openspec/changes/add-candidate-chooser/design.md`, open questions).
//!
//! Everything here counts characters, never bytes. The text is routinely Cyrillic, which is
//! two bytes per character in UTF-8; a byte-indexed window would overflow the row it was
//! measured for and panic on a character boundary while doing it.

/// A candidate cut down to a row, and what that cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The characters to draw. Never longer than the width asked for.
    pub text: String,
    /// Content exists before `text` and is not shown.
    pub elided_start: bool,
    /// Content exists after `text` and is not shown.
    pub elided_end: bool,
}

/// Fit `text` into `width` characters, keeping the part that differs from `compare_to` visible.
///
/// `compare_to` is normally the original selection: the difference between the two is exactly
/// what the user is being asked to judge. Passing the same string twice is not an error — there
/// is simply no changed region to favour, and the window starts at the beginning.
///
/// The window is placed by the same rule at every width, so a row and its expanded form show
/// the same region at different sizes rather than jumping to somewhere unrelated when opened.
pub fn compact(text: &str, compare_to: &str, width: usize) -> Preview {
    // A zero width is not a sensible request, but it is a reachable one — a window narrow
    // enough to round to nothing during a resize. One character is the smallest slice-safe
    // answer, and it keeps every caller free of a special case.
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();

    if chars.len() <= width {
        return Preview {
            text: text.to_string(),
            elided_start: false,
            elided_end: false,
        };
    }

    let (first, last) = changed_span(&chars, compare_to);

    // Centre the window on the changed span, then push it back inside the text. Clamping this
    // way — rather than centring on the midpoint — means an early change keeps the start of
    // the string visible instead of showing leading elision that hides nothing.
    let span_centre = first + (last.saturating_sub(first)) / 2;
    let half = width / 2;
    let start = span_centre
        .saturating_sub(half)
        .min(chars.len().saturating_sub(width));
    let end = (start + width).min(chars.len());

    Preview {
        text: chars[start..end].iter().collect(),
        elided_start: start > 0,
        elided_end: end < chars.len(),
    }
}

/// The first and last character positions at which `chars` differs from `compare_to`.
///
/// Falls back to the start of the text when the two are identical, so a caller comparing a
/// string against itself gets the beginning rather than a panic.
///
/// Found by trimming the matching ends: everything the two strings share at the front, and
/// everything they share at the back, is not the change. What is left between them is.
///
/// This is not a diff and does not try to be — it finds one contiguous region, so two separate
/// edits are reported as a single span covering both. That is the right answer for a preview,
/// which has one window to place.
///
/// It replaces a position-by-position comparison, which was correct only because layout
/// conversion maps character for character and so can never shift anything. Case conversion
/// shifts constantly: `snake_case` inserts separators, stripping removes characters, and German
/// `ß` uppercases to two characters. Under a positional comparison one inserted character makes
/// every character after it compare unequal, so the "changed region" becomes the whole rest of
/// the string and the window lands on its midpoint — which is nowhere near the change.
fn changed_span(chars: &[char], compare_to: &str) -> (usize, usize) {
    let other: Vec<char> = compare_to.chars().collect();

    let prefix = chars
        .iter()
        .zip(other.iter())
        .take_while(|(a, b)| a == b)
        .count();

    if prefix == chars.len() && prefix == other.len() {
        // Identical: nothing to favour, so favour the start.
        return (0, 0);
    }

    // Capped so the matching tail cannot reach back through the matching head. Without this,
    // a string like `aaa` against `aaaaa` counts the same characters at both ends and the
    // region comes out inverted.
    let room = (chars.len() - prefix).min(other.len() - prefix);
    let suffix = chars
        .iter()
        .rev()
        .zip(other.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(room);

    // `first` may sit one past the last shared character and `last` one before it: that is a
    // pure deletion, where the changed region in *this* string is empty and the position it
    // happened at is the only meaningful answer.
    let first = prefix.min(chars.len().saturating_sub(1));
    let last = (chars.len() - suffix).saturating_sub(1).max(first);
    (first, last)
}
