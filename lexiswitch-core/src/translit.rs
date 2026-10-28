//! Transliteration: rewriting Cyrillic text in Latin letters under a ruleset.
//!
//! A ruleset is a TOML text with one `[map]` of sequences of any length and a `[word_start]`
//! overlay. The longest sequence that matches at a position wins, so the order a ruleset is
//! written in cannot change what it means; a `[word_start]` value replaces the ordinary one when
//! the sequence begins a word; an empty value drops the sequence. Keys are lowercase only —
//! capitals are derived from the text being transliterated, so nobody writes `"Щ" = "Shch"`.
//!
//! Rulesets are compiled into a trie when loaded and are immutable afterwards. Nothing is cached
//! on disk: a ruleset is a few dozen entries and compiles faster than its file can be read
//! ([ADR-0017](../../docs/adr/0017-transliteration-third-transaction.md)).

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use crate::config::line_of_offset;

/// A compiled ruleset, ready to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruleset {
    id: String,
    name: String,
    source: String,
    target: String,
    fallback: Option<String>,
    /// Single characters whose value is empty. A word does not end at one of these: the
    /// apostrophe in `Знам'янка` is part of the word, so the `я` after it is not word-initial.
    dropped: Vec<char>,
    root: Node,
}

/// One step of the trie: what the sequence so far maps to, and where it can continue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Node {
    anywhere: Option<Box<str>>,
    word_start: Option<Box<str>>,
    /// Sorted by character, searched by binary search.
    children: Vec<(char, Node)>,
}

impl Node {
    fn child(&self, c: char) -> Option<&Node> {
        self.children
            .binary_search_by_key(&c, |(k, _)| *k)
            .ok()
            .map(|at| &self.children[at].1)
    }

    fn child_mut(&mut self, c: char) -> &mut Node {
        let at = match self.children.binary_search_by_key(&c, |(k, _)| *k) {
            Ok(at) => at,
            Err(at) => {
                self.children.insert(at, (c, Node::default()));
                at
            }
        };
        &mut self.children[at].1
    }

    /// The value that applies here, at a word start or elsewhere.
    fn value(&self, at_word_start: bool) -> Option<&str> {
        if at_word_start {
            self.word_start.as_deref().or(self.anywhere.as_deref())
        } else {
            self.anywhere.as_deref()
        }
    }
}

/// What a ruleset made of a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transliterated {
    pub text: String,
    /// Cyrillic letters no sequence covered — passed through, or replaced by the fallback. Worth
    /// showing: a result that looks complete but is not is how half-converted text gets pasted.
    pub untouched: usize,
}

/// The file as written, before anything is checked.
///
/// Unknown keys are **rejected**, unlike in the configuration file. Configuration tolerates them
/// so a file from an earlier version still starts; a ruleset has no earlier version, and a
/// misspelled `[wordstart]` that was silently ignored would look like a broken engine.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    id: Spanned<String>,
    name: String,
    source: Spanned<String>,
    target: Spanned<String>,
    #[serde(default)]
    fallback: Option<String>,
    #[serde(default)]
    map: BTreeMap<Spanned<String>, String>,
    #[serde(default)]
    word_start: BTreeMap<Spanned<String>, String>,
}

/// A ruleset that could not be used, and where the problem is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetError {
    message: String,
    line: Option<usize>,
}

impl RulesetError {
    fn at(text: &str, offset: usize, message: String) -> Self {
        Self {
            message,
            line: Some(line_of_offset(text, offset)),
        }
    }

    /// One-based line the problem is on, where that could be determined.
    pub fn line(&self) -> Option<usize> {
        self.line
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for RulesetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for RulesetError {}

impl Raw {
    /// Rules TOML itself cannot express, each reported at the line it concerns.
    fn validate(&self, text: &str) -> Result<(), RulesetError> {
        let fail = |at: &Spanned<String>, message: String| Err(RulesetError::at(text, at.span().start, message));

        let id = self.id.get_ref();
        if id.is_empty() || !id.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-')) {
            return fail(
                &self.id,
                format!("id {id:?} must be lowercase letters a-z, digits and hyphens, such as \"uk-kmu55\""),
            );
        }
        for (field, tag) in [("source", &self.source), ("target", &self.target)] {
            if !is_language_tag(tag.get_ref()) {
                return fail(
                    tag,
                    format!("{field} {:?} is not a language tag; write one such as \"uk-UA\"", tag.get_ref()),
                );
            }
        }
        for (table, entries) in [("map", &self.map), ("word_start", &self.word_start)] {
            for key in entries.keys() {
                if key.get_ref().is_empty() {
                    return fail(key, format!("[{table}] has an empty sequence; every entry needs at least one character"));
                }
                if key.get_ref().chars().any(char::is_uppercase) {
                    return fail(
                        key,
                        format!(
                            "[{table}] {:?} contains a capital; write sequences in lowercase only, capitals are derived from the text",
                            key.get_ref()
                        ),
                    );
                }
            }
        }
        Ok(())
    }
}

/// Whether `tag` has the shape of a BCP 47 tag: a language subtag of two to eight letters, then
/// any number of hyphen-separated subtags of one to eight letters or digits. Shape only — whether
/// the language exists is not this program's question.
fn is_language_tag(tag: &str) -> bool {
    let mut subtags = tag.split('-');
    let language = subtags.next().unwrap_or_default();
    (2..=8).contains(&language.len())
        && language.chars().all(|c| c.is_ascii_alphabetic())
        && subtags.all(|s| (1..=8).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric()))
}

impl Ruleset {
    /// Parse, validate and compile. Either the whole ruleset is good, or nothing is returned.
    pub fn parse(text: &str) -> Result<Self, RulesetError> {
        let raw: Raw = toml::from_str(text).map_err(|e| RulesetError {
            message: e.message().to_string(),
            line: e.span().map(|s| line_of_offset(text, s.start)),
        })?;
        raw.validate(text)?;

        let mut root = Node::default();
        let mut dropped = Vec::new();
        for (key, value) in &raw.map {
            let node = key.get_ref().chars().fold(&mut root, |n, c| n.child_mut(c));
            node.anywhere = Some(value.as_str().into());
            let mut chars = key.get_ref().chars();
            if let (Some(c), None, true) = (chars.next(), chars.next(), value.is_empty()) {
                dropped.push(c);
            }
        }
        for (key, value) in &raw.word_start {
            let node = key.get_ref().chars().fold(&mut root, |n, c| n.child_mut(c));
            node.word_start = Some(value.as_str().into());
        }
        dropped.sort_unstable();

        Ok(Self {
            id: raw.id.into_inner(),
            name: raw.name,
            source: raw.source.into_inner(),
            target: raw.target.into_inner(),
            fallback: raw.fallback,
            dropped,
            root,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The language the ruleset reads, as a BCP 47 tag.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The language the ruleset writes for, as a BCP 47 tag.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Transliterate `text`.
    ///
    /// At each position the longest sequence with a value applicable there wins; with none, the
    /// character is copied (or replaced by the fallback, if it is a Cyrillic letter). The cost is
    /// the length of the text times the length of the longest key, whatever the number of keys.
    pub fn apply(&self, text: &str) -> Transliterated {
        let chars: Vec<char> = text.chars().collect();
        let folded: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
        let mut out = String::with_capacity(text.len());
        let mut untouched = 0;

        let mut i = 0;
        while i < chars.len() {
            let at_start = self.starts_word(&folded, i);
            let mut node = &self.root;
            let mut best: Option<(usize, &str)> = None;
            for (j, &c) in folded.iter().enumerate().skip(i) {
                let Some(next) = node.child(c) else { break };
                node = next;
                if let Some(value) = node.value(at_start) {
                    best = Some((j + 1, value));
                }
            }
            match best {
                Some((end, value)) => {
                    self.push_cased(&mut out, &chars, &folded, i, end, value);
                    i = end;
                }
                None => {
                    let c = chars[i];
                    if is_cyrillic_letter(c) {
                        untouched += 1;
                        match &self.fallback {
                            Some(fallback) => out.push_str(fallback),
                            None => out.push(c),
                        }
                    } else {
                        out.push(c);
                    }
                    i += 1;
                }
            }
        }

        Transliterated { text: out, untouched }
    }

    fn drops(&self, folded: char) -> bool {
        self.dropped.binary_search(&folded).is_ok()
    }

    /// Whether position `i` begins a word: the nearest preceding character this ruleset does not
    /// drop is not a letter, or there is none.
    fn starts_word(&self, folded: &[char], i: usize) -> bool {
        folded[..i]
            .iter()
            .rev()
            .find(|&&c| !self.drops(c))
            .is_none_or(|c| !c.is_alphabetic())
    }

    /// Append `value` for the match `chars[start..end]`, in the case the match was written in.
    fn push_cased(
        &self,
        out: &mut String,
        chars: &[char],
        folded: &[char],
        start: usize,
        end: usize,
        value: &str,
    ) {
        if !chars[start].is_uppercase() {
            out.push_str(value);
            return;
        }
        let letters: Vec<char> = chars[start..end]
            .iter()
            .copied()
            .filter(|c| c.is_alphabetic())
            .collect();
        let match_in_capitals = letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase());
        if match_in_capitals || self.neighbour(chars, folded, start, end).is_some_and(char::is_uppercase) {
            out.extend(value.chars().flat_map(char::to_uppercase));
        } else {
            let mut rest = value.chars();
            if let Some(first) = rest.next() {
                out.extend(first.to_uppercase());
                out.push_str(rest.as_str());
            }
        }
    }

    /// The letter beside a match in the same word: the next one, or the previous one when the
    /// match ends the word. Dropped characters are looked past, as they are for word starts.
    fn neighbour(&self, chars: &[char], folded: &[char], start: usize, end: usize) -> Option<char> {
        let kept = |&(_, &c): &(usize, &char)| !self.drops(c);
        let next = folded.iter().enumerate().skip(end).find(kept).map(|(at, _)| chars[at]);
        if let Some(c) = next.filter(|c| c.is_alphabetic()) {
            return Some(c);
        }
        let previous = folded[..start].iter().enumerate().rev().find(kept).map(|(at, _)| chars[at]);
        previous.filter(|c| c.is_alphabetic())
    }
}

/// The rulesets compiled into the program, in the order they are offered: Ukrainian, then
/// russian.
///
/// They are TOML like any user ruleset and go through the same parser, so a built-in is just a
/// ruleset that cannot be deleted. A user file with the same id replaces one.
pub fn builtins() -> Vec<Ruleset> {
    [
        include_str!("../rulesets/uk-kmu55.toml"),
        include_str!("../rulesets/ru-icao.toml"),
    ]
    .into_iter()
    .map(|text| Ruleset::parse(text).expect("a built-in ruleset is valid; the tests prove it"))
    .collect()
}

/// The character keys are compared against. Cyrillic lowercases one-to-one; a character whose
/// lowercase is several characters is left alone, since no key can contain it.
fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Whether `c` is a letter from one of the Unicode Cyrillic blocks — the only characters a
/// fallback may replace.
fn is_cyrillic_letter(c: char) -> bool {
    c.is_alphabetic()
        && matches!(c as u32,
            0x0400..=0x052F | 0x1C80..=0x1C8F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F | 0x1E030..=0x1E08F)
}

/// Every ruleset on offer, in the order the window lists them.
///
/// The built-ins come first, then user rulesets by name. A user ruleset whose id is a built-in's
/// takes that built-in's place, so overriding the Ukrainian scheme does not move it down the
/// list. Built whole and swapped whole: nothing ever sees a set half-updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rulesets {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    ruleset: Ruleset,
    /// The file it came from, or `None` for a built-in. How a failing file finds the version of
    /// itself that last loaded.
    file: Option<String>,
}

/// A ruleset file that was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetFailure {
    file: String,
    error: RulesetError,
}

impl RulesetFailure {
    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn error(&self) -> &RulesetError {
        &self.error
    }
}

impl std::fmt::Display for RulesetFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file, self.error)
    }
}

impl Rulesets {
    /// The built-ins alone: what is offered before the folder has been read, or when it is empty.
    pub fn builtin() -> Self {
        Self::build(&[], &Self { entries: Vec::new() }).0
    }

    /// The built-ins overlaid by user files, given as `(file name, contents)` — or, for a file
    /// that could not be read, `(file name, why)`.
    ///
    /// A file that is rejected or unreadable is reported, and the version of it in `previous` — the last that
    /// loaded from that file name — is kept in its place, so saving a mistake does not take a
    /// working ruleset away. Files are taken in file-name order, so when two share an id the same
    /// one always wins.
    pub fn build(
        files: &[(String, Result<String, String>)],
        previous: &Rulesets,
    ) -> (Self, Vec<RulesetFailure>) {
        let mut files: Vec<&(String, Result<String, String>)> = files.iter().collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));

        let mut users: Vec<Entry> = Vec::new();
        let mut failures = Vec::new();
        for (file, text) in files {
            let taken_by = |id: &str, users: &[Entry]| {
                users
                    .iter()
                    .find(|e| e.ruleset.id == id)
                    .and_then(|e| e.file.clone())
            };
            let read = text.as_ref().map_err(|why| RulesetError {
                message: format!("could not be read: {why}"),
                line: None,
            });
            let parsed = read.and_then(|text| Ruleset::parse(text)).and_then(|ruleset| match taken_by(&ruleset.id, &users) {
                Some(other) => Err(RulesetError {
                    message: format!("id {:?} is already used by {other}", ruleset.id),
                    line: None,
                }),
                None => Ok(ruleset),
            });
            match parsed {
                Ok(ruleset) => users.push(Entry {
                    ruleset,
                    file: Some(file.clone()),
                }),
                Err(error) => {
                    let kept = previous
                        .entries
                        .iter()
                        .find(|e| e.file.as_deref() == Some(file.as_str()))
                        .filter(|e| taken_by(&e.ruleset.id, &users).is_none());
                    users.extend(kept.cloned());
                    failures.push(RulesetFailure {
                        file: file.clone(),
                        error,
                    });
                }
            }
        }

        let mut entries: Vec<Entry> = builtins()
            .into_iter()
            .map(|ruleset| {
                match users.iter().position(|u| u.ruleset.id == ruleset.id) {
                    Some(at) => users.remove(at),
                    None => Entry { ruleset, file: None },
                }
            })
            .collect();
        users.sort_by(|a, b| {
            (a.ruleset.name.to_lowercase(), &a.file).cmp(&(b.ruleset.name.to_lowercase(), &b.file))
        });
        entries.extend(users);

        (Self { entries }, failures)
    }

    /// The rulesets in the order they are offered.
    pub fn iter(&self) -> impl Iterator<Item = &Ruleset> {
        self.entries.iter().map(|e| &e.ruleset)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
