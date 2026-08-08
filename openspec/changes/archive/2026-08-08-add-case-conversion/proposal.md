## Why

LexiSwitch already reads a selection, shows what each transform would produce, and replaces
the text on an explicit commit. That machinery is built, verified across the application
matrix, and serves exactly one transform family. Changing the **case** of selected text —
`UPPERCASE`, `snake_case`, `Words Capitalized` — is a second family that wants the same
transaction and currently sends the user to a web tool or an editor plugin.

It is not, however, a second family of *candidates*. Any text at all yields layout
candidates, and any text at all can be re-cased, so a single merged list would carry roughly
eight irrelevant rows out of fifteen on every invocation, with no signal in the characters to
say which eight. [ADR-0014](../../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)
records why this becomes a second front door instead.

## What Changes

- A **second global shortcut**, defaulting to `Win+Ctrl+Pause`, opens a case window. The
  existing shortcut and chooser are untouched.
- **Fourteen case styles** in a fixed list, in three groups: `As typed`, `Invert case`,
  `lowercase`, `UPPERCASE` — then `camelCase`, `snake_case`, `dot.case`, `kebab-lowercase`,
  `PascalCase`, `SCREAMING_SNAKE_CASE`, `Capitalized_Snake_Case`, `KEBAB-UPPERCASE` — then
  `First word capitalized`, `Words Capitalized`.
- **Six strip toggles** on digit accelerators `1`–`6`: punctuation, digits, and two mutually
  exclusive pairs — spaces remove/normalize, lines join/remove. Modelled as two three-valued
  settings so the exclusivity is structural. Toggles start clear on every invocation.
- **Dimming, not filtering**, as the single rule for "this would change nothing", applied to
  rows and toggles alike. The list never changes shape. This is deliberately the opposite of
  `candidate-chooser`, where no-op candidates are dropped.
- **Frequency memory**: the three most-applied styles are copied to the top of the list,
  ordered by use count, fixed while the window is open, persisted to a new state file, and
  resettable in place with `Ctrl+R`.
- **Per-binding hotkey registration**. Registration is currently all-or-nothing; with two
  shortcuts that would let a conflict on the case shortcut silently disable layout
  conversion.
- **Preview windowing becomes insertion-tolerant**. `changed_span` is positional today and
  documents itself as wrong where characters are inserted or removed — which is what
  `snake_case` and `ß`→`SS` do.

Not breaking. Existing configuration files stay valid; the new shortcut key is optional and
defaulted, and unknown keys are already reported rather than rejected.

## Capabilities

### New Capabilities
- `case-conversion`: the case transaction — invocation, the style list and its grouping, the
  strip toggles and their semantics, the dimming rule, the frequency memory and its storage,
  and what commit and cancel guarantee.

### Modified Capabilities
- `agent-control`: the agent now carries **two** shortcuts rather than one. Registration
  becomes per-binding, a conflict must name which binding is unavailable while stating that
  the other still works, and the tray gains a command for the second transaction. Also
  introduces a second file on disk beside the configuration file
  ([ADR-0015](../../../docs/adr/0015-usage-memory-outside-configuration.md)), which the
  privacy requirements must account for.

`candidate-chooser` and `layout-conversion` are **not** modified. Keeping them frozen is the
point of ADR-0014.

## Impact

**New.** `lexiswitch-core/src/casing.rs` (segmentation, styles, strips, the dimming
predicate), `lexiswitch-core/src/usage.rs` (counts and ordering, text form only),
`lexiswitch-win/src/casewindow.rs`.

**Modified.** `lexiswitch-core`: `preview.rs` (`changed_span`), `config.rs` (the second
binding), `state.rs` (two shortcuts). `lexiswitch-win`: `hotkey.rs` (per-binding
registration, `HotkeyId` cleanup), `winui.rs` (receives the armed-foreground handshake and
the drawing helpers lifted out of `chooser.rs`), `tray.rs` (second command). `lexiswitch`:
`worker.rs` (the case job and ownership of the usage memory), `configwatch.rs` (state file
path), `main.rs` (wiring), `config.example.toml`.

**Unmodified, deliberately.** `chooser.rs`, `candidates.rs`, `convert.rs`, `transaction.rs`,
`selection.rs`, `inject.rs`, `clipboard.rs`.

**Dependencies.** None added. No new crates, so the no-network gate is unaffected.

**Budgets.** Binary is 0.63 MB against a 5 MB limit; one window and two pure modules do not
threaten it. Private working set is unaffected — the case window allocates no COM object and
holds no automation interface.

**Risk.** `Win+Ctrl+Pause` must be proven registrable: `MOD_WIN` parses already, but Windows
reserves Win-key combinations and `Win+Pause` itself opens System properties. A default that
cannot register would ship the product in a conflict state on first run.
