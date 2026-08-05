# ADR-0001: Windows-first, with an OS-free core

## Status

Accepted

## Context

LexiSwitch replaces Punto Switcher for a single user on Windows 11. The initial brief
named Windows 11 as the target, but also collected research on macOS event taps and
Linux evdev, leaving it ambiguous whether the first release should be cross-platform.

Cross-platform input handling is not a thin abstraction. Each platform has a different
primitive (`RIDEV_INPUTSINK`, `CGEventTapCreate`, `XRecord`, `evdev`), a different
privilege model, and a different set of failure modes. Wayland in particular has no
portable global-input path at all.

At the same time, the conversion logic itself — mapping characters through physical key
positions between layout tables — has nothing to do with any operating system, and is
where nearly all the interesting behavior and nearly all the bugs live.

## Decision

Implement **only the Windows driver**, but structure the codebase so the platform layer
is a replaceable boundary rather than a pervasive concern.

Three crates:

- `lexiswitch-core` — layout tables, conversion, keystroke buffer, target-layout cycling,
  config types, exclusion matching. No Win32, `#![forbid(unsafe_code)]`, compiles and
  tests on any host.
- `lexiswitch-win` — every Win32 call and every `unsafe` block.
- `lexiswitch` — the binary: wiring, config load and hot-reload, threading.

## Consequences

**Positive.** The bulk of the logic is unit-testable without a Windows machine, a GUI, or
a keyboard in the loop. `unsafe` is confined to one crate and auditable as a set. Adding
a macOS or Linux driver later means writing one crate against an existing interface, not
untangling platform code from logic. Small, focused crates are also easier to reason
about and to modify safely.

**Negative.** Three crates is more ceremony than a single binary needs today, and the
core/platform interface will need at least one revision once a second platform is real —
an interface designed against one implementation is a guess. Accepted: the guess is
cheap, and the testability is worth it on its own.

## Alternatives Considered

**Single crate, modules only.** Less ceremony, but nothing enforces the boundary. In
practice Win32 types leak into logic code, and `unsafe` spreads. Rejected because the
enforcement is the point.

**Cross-platform from day one.** Triples the platform work and the testing matrix for a
tool with one user on one OS. Also weakens the security argument: the "cannot intercept"
property this project is built on (ADR-0003) is clean on Windows, but on macOS both
listen-only and intercepting event taps require the same Accessibility grant, and on
Wayland both bottom out at `/dev/input` access that permits reads *and* writes. Designing
around a portable version of that claim would mean designing around a claim that isn't
portable. Rejected.

## Related

- [ADR-0002](0002-rust-as-implementation-language.md) — language choice
- [ADR-0003](0003-raw-input-over-low-level-hook.md) — the Windows observation mechanism
- [Change: add-manual-layout-conversion](../../openspec/changes/add-manual-layout-conversion/design.md) — component layout
