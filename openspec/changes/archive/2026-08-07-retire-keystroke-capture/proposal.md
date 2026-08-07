## Why

Last-word correction requires knowing what was typed, which requires observing typing, which
requires a buffer, which requires everything built to contain that buffer — password-field
detection, an excluded-application list, a pause state, wiping on focus change and on
`Enter`/`Tab`/`Escape`, an idle timeout, page-locking and zeroization. That is the largest and
least auditable subsystem in the program, and the product pivot retires the one feature it
serves ([ADR-0011](../../../docs/adr/0011-no-keyboard-observation.md)).

Multi-tap cycling goes with it. It depends on state the user cannot see, on timing, and on an
installed-layout order nobody chose; and on a selection it cannot work at all, because after a
replacement nothing is selected
([ADR-0012](../../../docs/adr/0012-single-chooser-transaction.md)).

This change is deliberately separable from the candidate chooser that replaces them. Selection
conversion already works, so removing capture leaves a smaller, working, single-purpose tool
rather than a long broken window.

## What Changes

- **BREAKING** Remove the last-word correction hotkey and its entire supporting subsystem:
  Raw Input registration, the page-locked ring buffer, and word extraction from key positions.
- **BREAKING** Remove multi-tap cycling, the multi-tap window, and all repeated-press
  semantics.
- **BREAKING** Remove the clipboard-conversion hotkey. Clipboard use is retained only as an
  internal transport for reading and replacing a selection.
- **BREAKING** Remove password-field detection, the excluded-application list, and the
  pause/resume state. These exist to make capture safe; with no capture they are not
  simplified but unnecessary.
- **BREAKING** Remove per-character text synthesis (`inject::replace`) and the synthetic-input
  marker that existed only so Raw Input could ignore this program's own injection.
- Rename the tray's pause/resume to enable/disable, which no longer refers to capture.
- Reduce configuration to what remains, with a defined and tested migration policy for config
  files written by the current version.
- Prove mechanically that no keyboard-observation capability survives in the release binary.

Selection conversion, layout-table derivation, and position-based conversion are untouched.

## Capabilities

### New Capabilities

None. This change only removes and narrows.

### Modified Capabilities

- `keystroke-capture`: every requirement is removed. The capability ceases to exist — nothing
  observes the keyboard, so there is no capture to bound, contain, or wipe.
- `manual-correction`: the last-word, clipboard-conversion and repeated-press-cycling
  requirements are removed. Selection conversion remains, and remains the only conversion
  the product offers.
- `agent-control`: the password-field-protection requirement is removed with the detection it
  governed; tray pause/resume becomes enable/disable; the configuration requirement narrows
  and gains a migration obligation.

`layout-conversion` is unchanged.

## Impact

**Deleted outright** — `lexiswitch-win/src/rawinput.rs`, `lexiswitch-win/src/pagelock.rs`,
`lexiswitch-core/src/buffer.rs`, `lexiswitch-core/src/cycle.rs`, and the core test files
`buffer.rs`, `cycling.rs`, `extraction.rs`, `invalidation.rs`.

**Narrowed** — `lexiswitch-win/src/focus.rs` loses `PasswordGate`, `inspect`, `FocusInfo` and
`Detection`, keeping `Uia` and `process_name`; `lexiswitch-win/src/inject.rs` loses `replace`
and `SYNTHETIC_MARKER`, keeping the paste and copy chords and `FocusGuard`;
`lexiswitch/src/main.rs` loses `WM_INPUT` handling and the buffer, gate, tap and correction
state; `lexiswitch/src/worker.rs` loses `Job::Replace` and `Job::ClipboardOnly`;
`lexiswitch-core/src/config.rs` loses the hotkey, buffer and exclusion settings that no longer
have referents.

**Configuration is a breaking change for existing users.** `deny_unknown_fields` is currently
set, so a config file written by the current version fails validation outright against the new
schema. The migration policy is part of this change, not an afterthought.

**Endpoint protection is an open experiment, not a claimed benefit.** The release build is
currently blocked and quarantined by CrowdStrike with a generic verdict and no attribution, and
every other suspicious attribute of that binary survives this change. Re-testing after removal
is how the question gets answered; see `docs/manual-tests.md`.

**Superseded decisions** — [ADR-0003](../../../docs/adr/0003-raw-input-over-low-level-hook.md)
in part, [ADR-0005](../../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md) and
[ADR-0009](../../../docs/adr/0009-multi-tap-layout-cycling.md) in full.
