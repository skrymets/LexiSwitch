## Context

See `proposal.md` — Why. The behaviour is specified in `specs/transliteration/spec.md` and the
`agent-control` delta; this document is how it is built and why it is built that way. The
decisions that outlive this change are recorded in
[ADR-0017](../../../docs/adr/0017-transliteration-third-transaction.md).

The design started from a generic requirements document for a "Cyrillic-to-Latin
transliteration subsystem". Several of its prescriptions are answered differently here because
LexiSwitch's constraints answer them first: those are called out under Decisions.

What exists and is reused unchanged:

- `Transaction` (`lexiswitch-win/src/transaction.rs`): read the selection, commit, cancel,
  restore the clipboard, keep a copied result.
- `winui`: window chrome, fonts, placement, the armed-foreground handshake, `Copied`.
- `preview::compact`: fitting a result into a row and marking what changed.
- Per-binding hotkey registration (`Hotkeys::register`) and the `Shortcut` / `Shortcuts` /
  `AgentState` model in `lexiswitch-core/src/state.rs`.
- The two-second mtime poll in `lexiswitch/src/configwatch.rs`.
- The shortcut recorder (`lexiswitch-win/src/recorder.rs`).

## Goals / Non-Goals

**Goals:**

- The engine is a pure function of a compiled ruleset and the text, testable in
  `lexiswitch-core` against the official example tables.
- A ruleset file is short and hard to get wrong: no priorities to order, no capitalised entries
  to duplicate.
- A broken user file cannot take down anything but itself.
- The layout and case paths are not touched.

**Non-Goals:**

- Latin→Cyrillic, or any other script pair. The ruleset format is not shaped to allow it.
- Combining transliteration with a case style in one step (`привіт світ` → `pryvit_svit`). It
  is a plausible later change; nothing here prevents it.
- Remembering the last-used ruleset. The list is short and its order fixed.
- Rulesets for scripts other than Cyrillic. `fallback` is defined in terms of the Cyrillic
  blocks.

## Decisions

### A third transaction, a new window

Transliteration gets its own shortcut and `translitwindow.rs`, by the same argument ADR-0014
made for case: any Cyrillic text has both layout candidates and transliterations, and nothing
in the characters says which the user wants. The window is modelled on `chooser.rs` — a plain
list of results, the same keys minus `Ctrl+Enter` — and shares only `Transaction`, `winui` and
`preview` with it.

*Alternatives:* a fifth group in the case window (cheap, but the case window's strip options
and promoted section mean nothing for transliteration, and it blurs what that window is for);
making `chooser.rs` generic over its rows (less code, but reopens the window ADR-0014 froze to
protect the main path); extracting a shared list window first (the right end state, but a
refactor of two verified windows inside a feature change — worth its own change if a fourth
list ever appears).

### One `[map]`, a `[word_start]` overlay, empty means dropped

The requirements document proposed four ordered arrays — combinations, contextual, flat
dictionary, omissions. They collapse:

- *Combinations* and the *flat dictionary* are the same thing at different lengths. Under
  longest-match there is nothing to order, so one table holds both and the order the user
  writes it in cannot matter. That removes the "overlapping prefix trap" by construction rather
  than by sorting.
- *Contextual* rules in Resolution No. 55 and every other common scheme have exactly two
  positions, word-initial and elsewhere. `elsewhere` is just the ordinary value, so only the
  word-initial value needs its own table.
- *Omissions* are sequences whose value is empty.

```toml
id       = "uk-kmu55"
name     = "Ukrainian (Resolution No. 55)"
source   = "uk-UA"
target   = "en-US"
fallback = "?"           # optional

[map]
"зг" = "zgh"
"я"  = "ia"
"ь"  = ""

[word_start]
"я" = "ya"
```

A `[word_start]` key does not need a `[map]` entry; where it has none, the sequence has no
value outside word-initial position and the characters fall through to shorter matches.

`direction` is dropped (out of scope). `fallback` is kept as an optional key the user asked for,
narrowed to Cyrillic letters so a ruleset cannot mangle Latin text or punctuation.

*Alternative:* the four-array form, literally. More to write, and every ruleset author would
have to learn which array wins when two match.

### TOML, parsed with the crate already in the tree

Rulesets are TOML for the same reasons configuration is (ADR-0008), and because the `toml`
crate is already a dependency. `lexiswitch-core/Cargo.toml` keeps JSON out of the shipped binary
on purpose; YAML would add a crate whose main implementation is unmaintained.

Parsing goes through `toml::Table` rather than `#[derive(Deserialize)]` on a struct, for two
reasons: unknown keys must be *rejected* (configuration ignores them for upgrade tolerance, but a
ruleset has no upgrade history and a silently ignored `[wordstart]` would look like a broken
engine), and every error must name a line — the existing `config.rs` helper that turns a key
path into a line number is reused.

### Compile at load, no disk cache

The requirements document asks for a compiled form cached on disk (rkyv or bincode), keyed by a
hash, so startup can skip parsing. Measured against this program:

- A ruleset is 40–100 entries. Parsing and building its trie is microseconds; reading the file
  from disk costs more than compiling it.
- A cache is a second file the program writes, and every file the program writes is one more
  thing to argue is free of user content.
- Invalidation is the requirements document's own risk #4. Without a cache there is nothing to
  invalidate.
- rkyv brings `unsafe` zero-copy access and has had soundness advisories; bincode is a
  dependency for nothing.

So rulesets are compiled when loaded — at startup and on every change the poll sees — into an
immutable value that is swapped in whole.

*Measured* (release build, 2026-10-28, averaged over 1,000 runs): parsing and compiling both
built-ins takes **58 µs**; transliterating a 10,800-character selection takes **0.76 ms**. A
cache would save tens of microseconds at startup — less than one frame of the tray icon being
drawn.

### A hand-written trie

```text
Node { anywhere: Option<Box<str>>, word_start: Option<Box<str>>, children: Vec<(char, Node)> }
```

Children are sorted by `char` and searched by binary search. Keys are stored lowercase, so the
walk folds each input character with `to_lowercase` (taking the single-char result; Cyrillic
folds one-to-one). At position *i* the scan walks as deep as the input allows and remembers the
deepest node with a value applicable at *i* — `word_start` if *i* begins a word and the node has
one, otherwise `anywhere`. It emits that value with derived casing and jumps past the match. If
nothing applies it emits the character, or `fallback` for an unmatched Cyrillic letter, and
moves one character.

Cost is O(L·k), k being the longest key — 2 for the built-ins. That is not literally the O(L)
the requirements document asks for, and the doc says so rather than claiming it; it is
independent of how many rules a ruleset has, which is the property that mattered.

*Alternative:* the `aho-corasick` crate in leftmost-longest mode. It finds matches but has no
notion of a match's value depending on its position, so word-start would need a second
automaton or post-filtering, and case folding would need either doubled patterns or a folded
copy of the input with an offset map. A forty-line trie does all of it directly.

### Word start looks back past dropped characters

"Begins a word" is decided by the nearest preceding character the ruleset does not drop. The
apostrophe in `Знам'янка` is a dropped character, so `я` after it is not word-initial and gives
`ia` — which is what the resolution prints. A hyphen is not dropped and not a letter, so
`Івано-Франківськ` starts a second word. This makes word-start a property of the ruleset as
well as the text, which is correct: whether `'` is part of a word depends on whether the scheme
treats it as one.

### Casing is derived from the match and its neighbour

Keys are lowercase; uppercase keys are a validation error so nobody writes `"Щ" = "Shch"` and
wonders why it is ignored. For an uppercase match the result is in capitals when the match
itself has two or more letters all in capitals, or when the adjacent letter in the same word —
the next one, or the previous one when the match ends the word — is a capital. Otherwise only
the first letter of the result is capitalised. A one-letter word (`Я`) has no neighbour and gets
title case, which is what an initial needs.

### Rulesets: built-ins overlaid by the folder, last good version per file

`lexiswitch-core/rulesets/uk-kmu55.toml` and `ru-icao.toml` are embedded with `include_str!` and
parsed by the same code as user files, so a built-in is just a ruleset that cannot be deleted —
and a test proves both parse and both reproduce their source tables.

A `Rulesets` value in the core holds the ordered list. The binary builds it from the built-ins
plus every `*.toml` in `%APPDATA%\LexiSwitch\transliteration\`. The folder is polled on the
existing configuration tick by comparing a sorted list of (name, size, modified time); any
difference rebuilds the whole set. For each file that fails, the version that last loaded from
that path is kept, and the failure is held as state for the tray. File IO stays in the binary;
the core takes `(file name, contents)` pairs where the contents are a `Result`, so a file that
could not be read — locked mid-save — keeps its last good version exactly as one that does not
parse.

A rejected file is a tray state, `AgentState::RulesetRejected`, as the `agent-control` delta
requires. It is applied by `with_rejected_rulesets` after `resolve` rather than as another
argument to it: it only ever refines the healthy state, so anything more blocking — and being
disabled — keeps its place, and the existing callers of `resolve` are unchanged.

### A defaulted shortcut that collides is left unbound

Adding a third shortcut creates a hazard the first two did not: a user who rebound `change_case`
to `Win+Alt+Pause` — the user this was designed with did — would, on upgrade, have a file whose
absent `transliterate` defaults to the same combination. The current duplicate check would
reject the file and the program would not start, breaking the existing promise that a file from
an earlier version always starts.

So `config.rs` records which hotkey keys were present in the file. Two bindings that collide are
still an error when both were written; when one was defaulted, the written one wins and the
defaulted one becomes unbound. `state.rs` gets `Shortcut::Unbound { binding, taken_by }` and
`AgentState::ShortcutUnbound`, which the tray reports ("The shortcut that transliterates has none:
its default, Win+Alt+Pause, already changes case in your configuration") and which counts as
needing attention, but not as a conflict with another application. A real conflict outranks it.
Registration skips an unbound shortcut entirely rather than attempting it.

`Shortcuts` grows a `translit` member and `described()` a third entry; `AgentState::Enabled`
carries the three bindings. The suggestion list for the recorder excludes all three.

## Risks / Trade-offs

- [An ASCII `'` used as a quotation mark is dropped under the Ukrainian ruleset] → it is
  dropped because the resolution drops the apostrophe, and the ASCII character is what most
  keyboards type for it. The built-in drops `'`, `’` and `ʼ`; a user who quotes with `'` can
  override the built-in without it. Documented in the example file.
- [`to_lowercase` of a non-Cyrillic character can yield several characters] → the walk only
  descends on the first; such characters are never keys (keys are validated as having no
  capitals and are compared folded), so the worst case is a missed match that passes the
  character through unchanged.
- [A third global shortcut is a third thing another application can hold] → registration is
  already per-binding, the tray command works regardless, and the recorder handles it.
- [A user's ruleset folder grows large] → each file is a few kilobytes and polled by metadata
  only; the content is read only when the listing changes.
- [Another ~600 lines of window code resembling `chooser.rs`] → accepted for zero regression
  risk to the main path; a shared list window is the follow-up if a fourth one appears.

## Migration Plan

Additive. Existing configuration files keep working, including ones that already bind
`Win+Alt+Pause` (see the collision rule). The release is a MINOR bump under ADR-0016. Rollback
is the previous binary; it ignores `transliterate` as an unknown key and never looks at the
folder.
