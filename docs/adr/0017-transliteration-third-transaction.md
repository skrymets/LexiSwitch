# ADR-0017: Transliteration is a third transaction, with rulesets compiled at load

## Status

Accepted. Supersedes nothing. Extends [ADR-0014](0014-case-conversion-is-a-separate-transaction.md)'s
rule — one invocation, one window, one kind of result — to a third kind.

## Context

Ukrainian and russian text regularly has to be written in Latin letters, under a scheme with
rules that are easy to get wrong by hand: `зг` is `zgh`, `я` is `ya` at the start of a word and
`ia` inside one, the soft sign and the apostrophe disappear. LexiSwitch already reads a
selection, previews transforms of it and replaces it on an explicit commit, so the expensive
part exists.

The work began from a generic requirements document for a "transliteration subsystem". It
proposed four ordered rule arrays (combinations, contextual rules, a flat dictionary,
omissions), JSON or YAML rule files, a compiled form cached on disk with rkyv or bincode and
invalidated by a hash, and an Aho–Corasick or radix-trie matcher. Each was weighed against what
this program already is.

Three facts decided most of it:

- **Any Cyrillic selection has both layout candidates and transliterations**, and nothing in the
  characters says which the user wants — the argument ADR-0014 made for case, unchanged.
- **A ruleset is small.** The official Ukrainian scheme is about forty entries. Parsing and
  compiling it costs microseconds, less than reading the file.
- **Every file the program writes has to be argued free of user content**, and the program
  writes exactly one today (ADR-0015).

## Decision

**Transliteration is its own transaction**: its own shortcut (`[hotkeys] transliterate`, default
`Win+Alt+Pause`), its own window, its own tray command. It shares `Transaction`, `winui` and
`preview` with the other two and nothing else. The window lists every ruleset every time and
dims those that would change nothing, as the case window does with styles.

**A ruleset is a TOML file with one `[map]` and a `[word_start]` overlay.** Sequences of any
length live in one table and the longest match wins, so the order entries are written in cannot
matter. A value used only at the start of a word goes in `[word_start]`; the ordinary value is
the one in `[map]`. A dropped character is a sequence whose value is empty. Keys are lowercase
and capitalisation is derived from the text. An optional `fallback` replaces Cyrillic letters
no sequence covers.

**Two rulesets are built in** — Ukrainian per Cabinet of Ministers Resolution No. 55 (2010) and
russian per ICAO Doc 9303 — and users add or replace rulesets as files in
`%APPDATA%\LexiSwitch\transliteration\`. A file that fails validation is rejected whole, reported
with its line, and replaced by its last good version; other files are unaffected.

**Rulesets are compiled when loaded, into an immutable in-memory trie, and nothing is cached on
disk.** The program never writes to the rulesets folder.

**The matcher is a hand-written trie**, not the `aho-corasick` crate.

**A shortcut left at its default that collides with one the user wrote is left unbound and
reported**, rather than rejecting the configuration file. Two bindings the user wrote that
collide are still an error.

## Consequences

**Positive.** The layout and case paths are not touched. A ruleset is short enough to read in
full and cannot be written in an order that changes its meaning. There is no cache to go stale,
no second program-written file, and no new dependency. The engine is a pure function in
`lexiswitch-core`, tested against the published example tables of both schemes. A
configuration file that rebound another shortcut to `Win+Alt+Pause` keeps starting after the
upgrade.

**Negative.** A third global shortcut is a third thing another application can hold. The window
is a third list window of similar code; a shared list window is the refactor to do if a fourth
appears. Rulesets are an exception to "one file describes all configurable behaviour", which the
`agent-control` specification now states. Under the Ukrainian scheme an ASCII `'` is dropped
even where it was meant as a quotation mark, because it is also what keyboards type for the
apostrophe the scheme drops; a user ruleset can override that.

## Alternatives Considered

- **A group in the case window.** Cheapest, but the case window's strip options and promoted
  styles mean nothing for transliteration, and it would blur what that window is for.
- **Rows among the layout candidates.** The mixture ADR-0014 rejected.
- **Four ordered rule arrays.** Combinations and single letters are one thing at different
  lengths; under longest-match there is nothing to order. "Elsewhere" is just the ordinary value.
  Omission is an empty value. Four concepts become two tables.
- **JSON or YAML.** The shipped binary deliberately contains no JSON parser; YAML's main Rust
  implementation is unmaintained. TOML is already parsed for configuration.
- **A compiled cache on disk (rkyv / bincode, hash-invalidated).** It would save microseconds,
  and cost a program-written file, an invalidation path that can fail, and a dependency — rkyv
  with `unsafe` zero-copy access and a history of soundness advisories. The requirements
  document's own list of risks names cache invalidation; not having a cache removes it.
- **The `aho-corasick` crate.** It finds the longest match but has no notion of a match's value
  depending on position, and case-insensitive matching would need doubled patterns or a folded
  copy of the text with an offset map. A trie of a few dozen lines does all of it directly.
  Its cost is O(L·k), k being the longest key, independent of the number of rules.
- **Rejecting a configuration whose defaulted shortcut collides.** The existing duplicate rule,
  applied unchanged. It would stop the program starting for a user who had rebound case to
  `Win+Alt+Pause`, which breaks the promise that a file from an earlier version always starts.

## Related

- [ADR-0008](0008-tray-and-toml-configuration.md) — TOML configuration
- [ADR-0014](0014-case-conversion-is-a-separate-transaction.md) — the separate-transaction rule
- [ADR-0015](0015-usage-memory-outside-configuration.md) — the one program-written file
- [ADR-0016](0016-semantic-versioning.md) — this is a MINOR change
- `openspec/changes/add-transliteration/`
