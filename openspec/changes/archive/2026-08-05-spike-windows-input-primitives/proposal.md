## Why

Three decisions in `docs/adr/` rest on assumptions about Windows APIs that nobody has
verified on this machine. Two of them (ADR-0003, ADR-0004) are load-bearing for the whole
project, and discovering they are wrong halfway through implementation would mean
rewriting work already done.

The assumptions:

1. `RIDEV_INPUTSINK` delivers keystrokes typed into elevated windows to an unelevated
   process. Low-level hooks do not. If Raw Input matches that restriction, it is an
   accepted limitation — but the project should know before it ships, not after.
2. `WM_HOTKEY` messages carry timing precise enough to distinguish tap from double-tap
   from triple-tap. All of ADR-0009's cycling and undo behavior depends on it.
3. `GetKeyboardLayoutList` plus `ToUnicodeEx` can produce complete
   `(scancode, shift) → char` tables for uk, en-US, and de-QWERTZ, including sane
   handling of German dead keys. ADR-0004 chose runtime-derived tables over hardcoded
   ones on the strength of this.

Assumption 3 is the most likely to break and the most expensive to be wrong about, since
a failure invalidates ADR-0004 outright. It runs first.

This change produces no shipped behavior. It answers three questions and generates the
layout fixtures that every `lexiswitch-core` test will depend on.

## What Changes

- Add `spikes/` containing three throwaway binaries, excluded from the shipped workspace.
- Record each spike's outcome in `docs/spikes/2026-08-05-windows-input-primitives.md`.
- Emit frozen JSON layout fixtures for uk, en-US, and de to
  `lexiswitch-core/tests/fixtures/`, for use by later changes.
- Where a spike contradicts a recorded decision, write a superseding ADR rather than
  quietly amending the existing one.

No production code. No dependency on this change's artifacts beyond the fixtures and the
recorded findings.

## Capabilities

### New Capabilities

None. This change adds no behavior to the product.

### Modified Capabilities

None. `skip_specs: true` is set in `.openspec.yaml`: specs describe behavior, and this
change deliberately produces none.

## Impact

- **New:** `spikes/spike1_uipi_reach/`, `spikes/spike2_hotkey_timing/`,
  `spikes/spike3_layout_tables/` — each a standalone `cargo` binary, not workspace members.
- **New:** `docs/spikes/2026-08-05-windows-input-primitives.md` — findings.
- **New:** `lexiswitch-core/tests/fixtures/{uk,en_us,de}.json` — frozen layout tables.
- **Dependencies introduced for evaluation:** the `windows` crate, feature-scoped to
  `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_WindowsAndMessaging`,
  `Win32_UI_Input_Ime`. Feature selection carries directly into `lexiswitch-win`.
- **At risk:** [ADR-0003](../../../docs/adr/0003-raw-input-over-low-level-hook.md) and
  [ADR-0004](../../../docs/adr/0004-runtime-derived-layout-tables.md). A negative result
  on spike 3 blocks `add-manual-layout-conversion` until ADR-0004 is resolved.
- **Not affected:** no product code exists yet.
