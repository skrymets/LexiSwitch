//! Which conversions are worth offering for a piece of selected text.
//!
//! The old model asked the wrong question. It took the conversion's source from whichever
//! layout was active in the foreground window, which is a fact about the present, not
//! evidence about text that may have been typed at any time — so selecting mistyped text
//! after switching layouts silently did nothing at all.
//!
//! This asks the right one: given these characters and these installed layouts, which
//! reinterpretations are possible? Every ordered pair of distinct layouts is tried, and the
//! ones that change nothing fall away on their own — a source that cannot produce the
//! selected characters passes all of them through unchanged, so its result *is* the original
//! and it is discarded without needing to be recognised as implausible
//! ([ADR-0012](../../docs/adr/0012-single-chooser-transaction.md)).
//!
//! Nothing here knows about Windows, the foreground window, or the user. It is a pure
//! function of the text and the tables, which is what makes it testable against the frozen
//! fixtures.

use crate::convert::{convert, convert_lossy};
use crate::layout::LayoutTable;

/// One reinterpretation of the selected text, ready to be shown and chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The layout that could have produced the original characters.
    pub from: String,
    /// The layout the key positions are reinterpreted under.
    pub to: String,
    /// What the selection becomes.
    pub text: String,
    /// How many characters came through untouched, because they map to no key in the source
    /// or the position is dead in either layout. Worth showing: a partial conversion that
    /// looks complete is how a user ends up with half-corrected text and no idea.
    pub unchanged: usize,
}

/// Every distinct conversion of `text` that changes it, best first.
///
/// Returns empty when nothing changes the text — a real answer, and a different one from a
/// failure. Callers are expected to say so rather than showing an empty list.
///
/// `active` is the layout currently in force in the focused window, if known. It is used as
/// a **weak tiebreak on the target only**, and never to choose a source — treating it as
/// evidence of what the text was typed under is the mistake this whole module replaces. It
/// earns its place because the strong signals routinely tie: Cyrillic identifies Ukrainian as
/// the source outright and then says nothing about whether the user wants English or German
/// back, and getting that wrong preselects the wrong candidate in the commonest case there is.
pub fn generate(text: &str, layouts: &[LayoutTable], active: Option<&str>) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();

    for from in layouts {
        for to in layouts {
            if from.name() == to.name() {
                continue;
            }
            // `convert` reports "nothing changed" as None, which is exactly the filter this
            // needs: a source that cannot represent these characters produces the original.
            let Some(converted) = convert(text, from, to) else {
                continue;
            };

            let candidate = Candidate {
                from: from.name().to_string(),
                to: to.name().to_string(),
                unchanged: unchanged_count(text, &converted),
                text: converted,
            };

            // Deduplicate on result *and* target, never on result alone. Two sources can
            // produce the same visible text while naming different targets, and the target is
            // what an apply-and-switch activates — collapsing them would silently remove a
            // choice the user is being asked to make.
            if !found
                .iter()
                .any(|c| c.text == candidate.text && c.to == candidate.to)
            {
                found.push(candidate);
            }
        }
    }

    rank(text, layouts, active, &mut found);
    found
}

/// How many characters the conversion left alone.
fn unchanged_count(original: &str, converted: &str) -> usize {
    original
        .chars()
        .zip(converted.chars())
        .filter(|(a, b)| a == b)
        .count()
}

/// Order the candidates, most plausible first.
///
/// The signals are deterministic and self-contained: no dictionary, no language model, no
/// data file. That is a hard constraint, and it has a consequence worth stating rather than
/// hiding — Cyrillic identifies Ukrainian outright, while Latin text never separates en-US
/// from de-DE, so for Latin-to-Latin pairs this ordering cannot be better than arbitrary. It
/// is at least *stable*, and the chooser shows every candidate so the user's eye decides.
fn rank(text: &str, layouts: &[LayoutTable], active: Option<&str>, found: &mut [Candidate]) {
    let total = text.chars().count().max(1);

    found.sort_by(|a, b| {
        let a_unique = uniquely_identifies(&a.from, text, layouts);
        let b_unique = uniquely_identifies(&b.from, text, layouts);

        // A source that alone can produce these characters is the strongest signal available.
        b_unique
            .cmp(&a_unique)
            // Then how much of the text that source actually accounts for.
            .then_with(|| coverage(&b.from, text, layouts).cmp(&coverage(&a.from, text, layouts)))
            // Then the conversion that touched more of the selection.
            .then_with(|| (a.unchanged * 100 / total).cmp(&(b.unchanged * 100 / total)))
            // Then, weakly, the target that matches where the user is already typing. Only
            // reached when the signals above tie, which for two candidates from the same
            // source is most of the time.
            .then_with(|| {
                let a_active = Some(a.to.as_str()) == active;
                let b_active = Some(b.to.as_str()) == active;
                b_active.cmp(&a_active)
            })
            // Finally, something total, so the order never varies between runs.
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.to.cmp(&b.to))
    });
}

/// Whether `name` is the only installed layout that can produce any of these characters.
fn uniquely_identifies(name: &str, text: &str, layouts: &[LayoutTable]) -> bool {
    let producers = layouts
        .iter()
        .filter(|t| t.name() == name || produces_any(t, text))
        .count();
    producers == 1 && layouts.iter().any(|t| t.name() == name && produces_any(t, text))
}

fn produces_any(table: &LayoutTable, text: &str) -> bool {
    text.chars()
        .any(|c| !c.is_whitespace() && table.position_of(c).is_some())
}

/// What proportion of the non-whitespace characters this source can produce, as a percentage.
fn coverage(name: &str, text: &str, layouts: &[LayoutTable]) -> usize {
    let Some(table) = layouts.iter().find(|t| t.name() == name) else {
        return 0;
    };
    let considered: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if considered.is_empty() {
        return 0;
    }
    let mapped = considered
        .iter()
        .filter(|c| table.position_of(**c).is_some())
        .count();
    mapped * 100 / considered.len()
}

/// Convert without filtering, for a caller that has already chosen a pair.
///
/// Exposed so the chooser renders every candidate from the original selection rather than
/// from another candidate's result, which is what stops repeated views compounding.
pub fn render(text: &str, from: &LayoutTable, to: &LayoutTable) -> String {
    convert_lossy(text, from, to)
}
