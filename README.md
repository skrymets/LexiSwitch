# LexiSwitch

An offline, auditable keyboard-layout correction utility for Windows 11.

Typing Ukrainian words while an English or German layout is active produces garbage like
`ауфегкуі`. LexiSwitch converts it back to what you meant to type.

## Why this exists

Punto Switcher solves this problem well, but it is a Yandex product. Installing a
global keystroke interceptor from a Russian-controlled company is not an acceptable
trade. LexiSwitch reimplements the one feature that matters, with a security posture
that is architectural rather than promised:

- **It cannot intercept your typing.** Keystrokes are observed through Windows Raw Input
  (`RIDEV_INPUTSINK`), an API with no capability to block or alter a keystroke — not
  through `SetWindowsHookEx`, which has exactly that capability. See
  [ADR-0003](docs/adr/0003-raw-input-over-low-level-hook.md).
- **No network capability.** Network-capable dependencies are denied by `cargo-deny`, and
  the deny list is tested against a real networking crate rather than assumed to work.
  There is no update checker and no telemetry.
- **Nothing derived from keystrokes reaches disk.** The keystroke buffer is bounded,
  page-locked, and zeroized.

## Status

Pre-implementation. Planning is complete; no code exists yet.

Planning is managed with [OpenSpec](https://github.com/Fission-AI/OpenSpec). Run
`openspec list` to see active changes, `openspec status --change <name>` for one of them.

- [`spike-windows-input-primitives`](openspec/changes/spike-windows-input-primitives/proposal.md)
  — answers three unverified Win32 assumptions and produces the frozen layout fixtures.
  Runs first; a negative result on layout-table extraction invalidates ADR-0004.
- [`add-manual-layout-conversion`](openspec/changes/add-manual-layout-conversion/proposal.md)
  — all of v1, across four capabilities: `layout-conversion`, `keystroke-capture`,
  `manual-correction`, `agent-control`.

Architecture decisions and their rejected alternatives live in
[`docs/adr/`](docs/adr/README.md). A recorded decision is changed by writing a superseding
ADR, not by editing it.

## Building

See [`docs/environment.md`](docs/environment.md) for what must be installed. Note that the
German keyboard layout is a hard test dependency, not optional tooling — layout tables are
derived from whatever Windows has installed, and German is the only layout in the target
set with dead keys.

## Scope

**v1** — manual hotkeys: convert the last typed word, convert the selection, convert the
clipboard. Repeated presses cycle through candidate layouts and wrap back to the
original, which is also how undo works.

**Later** — heuristic automatic switching, dictionary-backed automatic switching, a
settings GUI. See the spec.

## Target constraints

Windows 11, Rust, binary under 5 MB, private working set under 20 MB.
