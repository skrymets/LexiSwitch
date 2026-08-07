//! Which case styles get used, so the three used most can sit on top of the list.
//!
//! Kept beside the configuration file rather than in it
//! ([ADR-0015](../../docs/adr/0015-usage-memory-outside-configuration.md)): these counts change
//! on every apply, and rewriting the user's hand-authored file several times a day as a side
//! effect of ordinary use would fight the surgical-edit design that exists to protect it.
//! Configuration describes behaviour; this describes history, and they do not belong together.
//!
//! Text only. The binary does the file IO, the same split [`Config`](crate::config::Config) has,
//! so every rule worth getting right is testable without a filesystem.
//!
//! **What may be in here is closed.** Style identifiers this program defines, and integers.
//! Nothing derived from what the user typed or selected — no text, no results, no window names,
//! no timestamps. The claim is meant to be checkable by opening the file, which is only true
//! while [`render`](Usage::render) can write nothing else.

use std::collections::BTreeMap;

/// How often each style has been applied.
///
/// Ordering is by count, never by recency. A recency list reorders after every single use, so
/// the row the user is reaching for moves under their hand — which defeats the muscle memory
/// the promotion exists to build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    /// Keyed by [`Style::id`](crate::casing::Style::id), including identifiers this version does
    /// not recognise. Unknown ones take no part in ordering but are written back untouched, so
    /// running an older build once does not silently destroy a newer build's counts.
    counts: BTreeMap<String, u32>,
}

impl Usage {
    /// Note that a style was applied.
    pub fn record(&mut self, style: crate::casing::Style) {
        let entry = self.counts.entry(style.id().to_string()).or_insert(0);
        // Saturating rather than wrapping. Unreachable at human apply rates, and costing
        // nothing means the question never has to be revisited.
        *entry = entry.saturating_add(1);
    }

    /// Forget everything, as the window's in-place reset does.
    ///
    /// Unknown identifiers go too: the user asked to start again, and quietly keeping something
    /// they cannot see would make the reset a lie.
    pub fn reset(&mut self) {
        self.counts.clear();
    }

    /// Whether anything has been recorded that this version can act on.
    pub fn is_empty(&self) -> bool {
        self.ordered().is_empty()
    }

    /// The `n` most-applied styles, most first.
    ///
    /// Only styles that have actually been used. Padding to `n` with styles nobody has chosen
    /// would put arbitrary rows in the most valuable position on the screen.
    pub fn top(&self, n: usize) -> Vec<crate::casing::Style> {
        self.ordered().into_iter().take(n).collect()
    }

    /// Every recorded style this version knows, most-used first.
    ///
    /// Ties break by declared order, so the result never depends on which arrived first and
    /// never varies between runs.
    fn ordered(&self) -> Vec<crate::casing::Style> {
        let mut found: Vec<(u32, usize, crate::casing::Style)> = self
            .counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .filter_map(|(id, count)| {
                let style = crate::casing::Style::from_id(id)?;
                let declared = crate::casing::Style::ALL
                    .iter()
                    .position(|s| *s == style)
                    .unwrap_or(usize::MAX);
                Some((*count, declared, style))
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        found.into_iter().map(|(_, _, style)| style).collect()
    }

    /// Read counts from a file's text.
    ///
    /// Never fails. Missing, malformed or half-written text all mean the same thing — no memory
    /// yet — and there is nothing in this file worth refusing to start over.
    pub fn parse(text: &str) -> Self {
        #[derive(serde::Deserialize, Default)]
        struct Stored {
            #[serde(default)]
            case_styles: BTreeMap<String, toml::Value>,
        }

        let Ok(stored) = toml::from_str::<Stored>(text) else {
            return Self::default();
        };
        Self {
            counts: stored
                .case_styles
                .into_iter()
                .filter_map(|(id, value)| {
                    // Anything that is not a count is dropped rather than guessed at.
                    let n = value.as_integer()?;
                    Some((id, u32::try_from(n).ok()?))
                })
                .collect(),
        }
    }

    /// The text to write.
    pub fn render(&self) -> String {
        let mut out = String::from(HEADER);
        out.push_str(&format!("[{TABLE}]\n"));
        for (id, count) in &self.counts {
            out.push_str(&format!("{id} = {count}\n"));
        }
        out
    }
}

const TABLE: &str = "case_styles";

const HEADER: &str = "\
# LexiSwitch usage counts. Written by LexiSwitch; not a configuration file.
#
# How often each case style has been applied, so the three used most appear at the top of the
# case window. Delete this file, or use the reset inside that window, to start again — nothing
# here changes what LexiSwitch does.
#
# Style names and numbers, and nothing else. No text you typed or selected is recorded here,
# by LexiSwitch or by anything else. See docs/adr/0015-usage-memory-outside-configuration.md.

";
