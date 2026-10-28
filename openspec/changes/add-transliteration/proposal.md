## Why

Ukrainian or russian text often has to be written in Latin letters: a name for a form, a file
name, a URL slug, a transliterated address. Doing it by hand is slow and inconsistent, and the
official schemes have rules that are easy to get wrong — `зг` is `zgh`, `я` is `ya` at the start
of a word and `ia` inside one, the soft sign and the apostrophe disappear. LexiSwitch already
reads a selection, previews a transform and replaces it on an explicit commit; transliteration
is one more transform that wants exactly that machinery.

## What Changes

- A **third transaction**, transliteration, with its own shortcut (default `Win+Alt+Pause`), its
  own window and its own tray command. It never shares a list with layout candidates or case
  styles (ADR-0014's rule, extended — ADR-0017).
- **Rulesets** describe a Cyrillic→Latin scheme in TOML: one `[map]` of sequences of any length
  where the longest match wins, a `[word_start]` table of values used at the start of a word, an
  empty value for a dropped character, and an optional `fallback` for Cyrillic letters the
  ruleset does not cover. Capitalised forms are derived, never written.
- **Two built-in rulesets**: Ukrainian per Cabinet of Ministers Resolution No. 55 (2010), and
  russian per ICAO Doc 9303 (the passport scheme).
- **User rulesets** are TOML files in `%APPDATA%\LexiSwitch\transliteration\`, picked up without
  a restart. A user file with a built-in's id replaces it. A file that fails validation is
  reported with its line and keeps its last good version; the others are unaffected.
- Rulesets are **compiled at load** into an immutable in-memory structure. Nothing is cached on
  disk and the program writes nothing to that folder.
- A new configuration key `[hotkeys] transliterate`. A shortcut left at its default that
  collides with a binding the user set explicitly is left unbound and reported, rather than
  rejecting a configuration file that worked before the upgrade.
- Tray: **Transliterate selection…**, **Change transliteration shortcut…**, **Open
  transliteration folder**.

## Capabilities

### New Capabilities

- `transliteration`: invoking a transliteration of the selection, the window it is chosen in,
  what a ruleset may say and how it is applied (longest match, word start, dropped characters,
  fallback, derived casing), built-in and user rulesets and how a bad ruleset is handled, and
  that nothing from the transaction is retained.

### Modified Capabilities

- `agent-control`: the tray offers a transliteration command and a way to change its shortcut;
  a defaulted shortcut that collides with one the user set is left unbound and reported instead
  of rejecting the configuration.

## Impact

- **New code**: `lexiswitch-core/src/translit.rs` and its tests, the built-in rulesets under
  `lexiswitch-core/rulesets/`, `lexiswitch-win/src/translitwindow.rs`.
- **Changed code**: `lexiswitch-core/src/config.rs` (the new key and the collision rule),
  `lexiswitch-win/src/hotkey.rs`, `lexiswitch-win/src/tray.rs`, `lexiswitch/src/main.rs`,
  `lexiswitch/src/worker.rs`, `lexiswitch/src/configwatch.rs`.
- **Untouched**: `chooser.rs`, `candidates.rs`, `casewindow.rs`, `casing.rs`.
- **Dependencies**: none added. Rulesets are parsed with the `toml` the configuration already
  uses.
- **Packaging**: `transliteration.example.toml` ships in the release zip.
- **Docs**: ADR-0017, README, INSTALL.txt, config.example.toml, the manual checklist, CLAUDE.md.
- **Version**: adds a feature without breaking anything — a MINOR bump under ADR-0016.
