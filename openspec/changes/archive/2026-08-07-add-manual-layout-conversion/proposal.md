## Why

Typing Ukrainian words while an English or German layout is active produces garbage like
`ауфегкуі`. The fix today is to select the text, delete it, switch layout, and retype —
several times a day.

Punto Switcher solves this, but it is a Yandex product, and a global keystroke
interceptor from a russian-controlled company is not an acceptable thing to run. The
objection is not evidence of exfiltration; it is that `SetWindowsHookEx` *can* intercept
and alter every keystroke, and the vendor is not one whose assurances carry weight.

LexiSwitch v1 delivers the correction feature with a security property the original
cannot claim: it observes keystrokes through Raw Input, an API with no capability to
block or alter input, so "it cannot intercept your typing" is verifiable by reading the
imports rather than by trusting a promise.

v1 is manual only — nothing happens unless a hotkey is pressed. Automatic detection is
where the false-positive risk lives, and a tool that silently mangles a correct word, a
technical term, or a URL loses trust permanently. Manual conversion has no such failure
mode and exercises every component automatic switching will later need.

## What Changes

- Add a Cargo workspace of three crates: `lexiswitch-core` (no Win32, all logic),
  `lexiswitch-win` (all Win32 FFI, all `unsafe`), `lexiswitch` (wiring and event loop).
- Derive keyboard layout tables at startup from the layouts Windows actually has
  installed. No layout data compiled into the binary.
- Observe keystrokes via Raw Input `RIDEV_INPUTSINK` into a bounded 64-entry in-memory
  ring buffer that never reaches disk and never fills in password fields or excluded
  applications.
- Add three hotkey actions: convert the last typed word in place, convert the selection
  in place, convert the clipboard in place.
- Repeated presses within the multi-tap window cycle through candidate layouts and wrap
  back to the original. With two layouts installed this is double-tap undo, arriving as a
  consequence of the general rule rather than a special case.
- Add a tray icon (enable, pause, reload config, open config, quit) and a hand-edited
  TOML configuration file with hot reload.
- Enforce the no-network property mechanically via `cargo-deny`, not by convention.

Not in this change: automatic detection, dictionaries, a settings GUI, macOS or Linux
drivers, installer or code signing.

## Capabilities

### New Capabilities

- `layout-conversion`: Deriving layout tables from installed Windows layouts and
  converting text between them through physical key positions rather than alphabets.
- `keystroke-capture`: The bounded in-memory keystroke buffer — what it holds, when it is
  permitted to fill, and when it is wiped.
- `manual-correction`: The three hotkey-triggered actions, multi-tap layout cycling, text
  replacement, and post-correction layout switching.
- `agent-control`: Running as a background agent — tray presence and controls,
  configuration file, hot reload, and how failures reach the user.

### Modified Capabilities

None. `openspec/specs/` is empty; this is the first change to define behavior.

## Impact

- **New crates:** `lexiswitch-core`, `lexiswitch-win`, `lexiswitch`.
- **New files:** `Cargo.toml` (workspace), `deny.toml`, `config.example.toml`,
  `docs/manual-tests.md`.
- **Dependencies:** the `windows` crate with the feature set established by
  `spike-windows-input-primitives`; `serde` and `toml` for configuration; `zeroize` for
  buffer hygiene. No networking crate, enforced by `cargo-deny`.
- **Consumes:** frozen layout fixtures at `lexiswitch-core/tests/fixtures/`, produced by
  `spike-windows-input-primitives`.
- **Depends on:** `spike-windows-input-primitives` completing. A negative spike-3 result
  invalidates [ADR-0004](../../../docs/adr/0004-runtime-derived-layout-tables.md) and
  blocks this change until the layout strategy is redesigned.
- **Decisions:** implements [ADR-0001](../../../docs/adr/0001-windows-first-with-os-free-core.md)
  through [ADR-0009](../../../docs/adr/0009-multi-tap-layout-cycling.md).
- **Windows integration:** registers global hotkeys (fails loudly on conflict), reads the
  clipboard on the selection fallback path, and sends synthetic input via `SendInput`.
