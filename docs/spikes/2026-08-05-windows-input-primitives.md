# Spike findings: Windows input primitives

**Date:** 2026-08-05
**Change:** `openspec/changes/spike-windows-input-primitives`

Three Win32 assumptions underpin [ADR-0003](../adr/0003-raw-input-over-low-level-hook.md),
[ADR-0004](../adr/0004-runtime-derived-layout-tables.md), and
[ADR-0009](../adr/0009-multi-tap-layout-cycling.md). This document records what was
*observed*, not what was decided — an observation stays true, a decision can be superseded.
An ADR that changes as a result of this cites it.

## Answers

| Spike | Question | Answer |
|---|---|---|
| 3 | Can layout tables be derived at runtime, dead keys included? | **Yes.** ADR-0004 stands. Fixtures generated. |
| 1 | Does `RIDEV_INPUTSINK` reach into elevated windows? | **No.** Same UIPI restriction as a hook. Limitation accepted, recorded in ADR-0003. |
| 2 | Is `WM_HOTKEY` timing good enough for multi-tap, and are conflicts detectable? | **Yes to both.** 15.625 ms granularity; 500 ms confirmed. Conflicts fail with a readable error. |

**No decision was invalidated, so no superseding ADR was written** (task 5.3). ADR-0004 was
upheld by spike 3. ADR-0003's decision is unchanged — spike 1 resolved a question that ADR
had itself flagged as open, and the outcome was the one it pre-accepted. ADR-0009's 500 ms
default is confirmed rather than revised.

## Machine configuration

Every finding below is specific to this machine and this configuration. That is acceptable
— the product has one user on one machine — but the scope of the claim is what follows.

| | |
|---|---|
| OS | Microsoft Windows 11 Pro |
| Build | 10.0.26200.0 (25H2) |
| Toolchain | Rust 1.97.1 `stable-x86_64-pc-windows-msvc` |
| Session elevated | **No** — spikes run unelevated unless a task states otherwise |

Installed keyboard layouts, as reported by Windows:

| Language | Input method tip | HKL |
|---|---|---|
| English (United States) | `0409:00000409` | `0x04090409` |
| Ukrainian | `0422:00020422` | `0x04220422` (enhanced) |
| German (Germany) | `0407:00000407` | `0x04070407` |

German was installed on 2026-08-05, specifically for spike 3 — it is the only layout in the
target set with dead keys (`^`, `´`), which is the hazard the spike exists to expose.

---

## Spike 3 — layout table extraction

**Question:** Can `GetKeyboardLayoutList` plus `ToUnicodeEx` produce complete
`(scancode, shift) → char` tables for uk, en-US, and de-QWERTZ, including sane handling of
German dead keys?

**Why it matters:** [ADR-0004](../adr/0004-runtime-derived-layout-tables.md) chose
runtime-derived tables over hardcoded ones on the strength of this. A negative result
invalidates that decision outright and blocks `add-manual-layout-conversion`.

**Answer: yes.** Complete tables were extracted for all three layouts, dead keys behave
predictably once flushed, and the fixtures in `lexiswitch-core/tests/fixtures/` were
generated from this run. **ADR-0004 stands; no superseding ADR is required.**

### Dead keys, and the flush

The hazard is real and was reproduced deliberately rather than assumed. Probing German `^`
(scancode `0x29`) leaves the accent pending inside the layout, and the *next* probe returns
a combined character with no error of any kind:

```
probe 0x29 (^)            -> Dead
probe 0x12 (e), NO flush  -> Char("ê")     <- contamination
probe 0x12 (e), flushed   -> Char("e")
verdict: PASS - hazard reproduced, flush clears it
```

The flush is a `ToUnicodeEx` call with `VK_SPACE`, repeated until it returns `>= 0`. In
practice one call suffices, because space consumes the pending accent and returns the
spacing form. The spike flushes *before* every probe and again after any probe that reports
dead, so no probe can inherit state from its predecessor.

`ToUnicodeEx` returns `-1` for a dead key. Those positions are recorded as `"dead": true`
in the fixtures rather than storing whatever character a follow-up call produced — a dead
key has no single character, and inventing one would put a wrong answer into data every
later test treats as ground truth.

German dead-key positions found: `0x29` unshifted (`^`) and `0x0d` in **both** shift states
(`´` and `` ` ``). `0x29` shifted is `°`, which is not dead.

### Layouts as enumerated

```
HKL 0x04090409  langid 0x0409  en-US
HKL 0xf0a80422  langid 0x0422  uk-UA
HKL 0x04070407  langid 0x0407  de-DE
```

**The HKL high word is not always the layout identifier.** Ukrainian reports `0xf0a80422`,
not `0x04220422`, because the installed variant is Ukrainian (Enhanced). Only the low word
is the language identifier. Code that recognises a layout by comparing the full HKL will
silently fail to match it — mask with `& 0xFFFF`.

### Spot check against the physical keyboard

German is correct QWERTZ: `y`/`z` transposed (`0x15` → `z`, `0x2c` → `y`), `ß` on `0x0c`,
`ü ö ä` on `0x1a 0x27 0x28`, `§` on shift-`0x03`, and the ISO extra key `0x56` → `<`/`>`.
Ukrainian is correct, including `є` on `0x28`, `ї` on `0x1b`, and `ґ` on `0x56`.

Round-tripping through the fixtures behaves as the design expects:

```
uk -> en_us : 'ауфегкуі'   => 'features'
en_us -> uk : 'features'   => 'ауфегкуі'
```

### Known limitations of the extracted tables

- **AltGr is not probed.** Only unshifted and shifted states were captured, per task 2.3.
  German reaches `@ € µ ~ | { } [ ]` through AltGr, and none of those positions appear in
  `de.json`. Conversion involving them will pass the character through unchanged. This is a
  gap in coverage, not an error in what was captured; widening it means adding a third
  shift state to the probe and regenerating.
- **One machine, one moment.** The tables reflect the layouts installed here on
  2026-08-05. They are committed as frozen fixtures precisely so later tests do not depend
  on the machine they run on.
- **Scancode set is the alphanumeric and punctuation block only** — 48 positions, plus the
  ISO extra key. Function keys, the numeric keypad, and navigation keys are out of scope
  for conversion.

### `windows` crate features actually needed

`Win32_UI_Input_KeyboardAndMouse` and `Win32_Globalization`. Note that `HKL` lives in
`Win32_UI_Input_KeyboardAndMouse`, **not** in `Win32_UI_TextServices` as the change's task
2.1 assumed — that feature was listed initially and removed once it proved unnecessary.

---

## Spike 1 — UIPI reach

**Question:** Does `RIDEV_INPUTSINK` deliver keystrokes typed into elevated windows to an
unelevated process?

**Why it matters:** Low-level hooks do not cross that boundary. If Raw Input matches the
restriction it is an accepted limitation, but the project should know before it ships.

**Answer: no. Raw Input does not cross the elevation boundary.**

Run unelevated, with the spike observing:

- **Unelevated Notepad** — every keystroke delivered. Both edges, correct scancodes:
  ```
  scancode 0x14  vkey 0x54  flags 0x0000  down   foreground: Notepad.exe
  scancode 0x14  vkey 0x54  flags 0x0001  up     foreground: Notepad.exe
  ```
- **Elevated Notepad** — **no events at all.** Not delayed, not partial: silent.

`RIDEV_INPUTSINK` is therefore subject to the same UIPI restriction as a low-level hook. An
unelevated LexiSwitch is blind to typing in elevated windows.

This does not invalidate [ADR-0003](../adr/0003-raw-input-over-low-level-hook.md) — the
decision was to observe rather than intercept, and that stands. It adds a limitation, which
has been recorded there as a consequence along with its workaround and the cost of taking
it.

### Incidental observations

Both worth knowing before the buffer is written:

- **Auto-repeat arrives as repeated `down` events with no distinguishing flag.** Holding
  Shift produced fourteen identical `scancode 0x2a vkey 0x10 flags 0x0000 down` records
  before the matching `up`. Nothing in the event marks them as repeats. A 64-entry ring
  ([ADR-0005](../adr/0005-bounded-in-memory-keystroke-buffer.md)) would be filled by a
  single held key, evicting real typing — so the capture path in task 5.3 of
  `add-manual-layout-conversion` needs to either drop modifier keys or collapse repeats,
  and the task does not currently say which.
- **Key up and key down are distinguished only by `flags & 0x01`.** There is no separate
  message; `0x0000` is down, `0x0001` is up.

---

## Spike 2 — hotkey timing

**Question:** Do `WM_HOTKEY` message timestamps resolve finely enough to distinguish tap
from double-tap from triple-tap, and is 500 ms a workable multi-tap window?

**Why it matters:** All of [ADR-0009](../adr/0009-multi-tap-layout-cycling.md)'s cycling
and undo behavior depends on it.

### Conflict detection — answered

`RegisterHotKey` fails visibly and legibly when the combination is already held:

```
first  RegisterHotKey(Pause) -> ok
second RegisterHotKey(Pause) -> FAILED: Hot key is already registered. (0x80070581)
```

`0x80070581` is `HRESULT_FROM_WIN32(ERROR_HOTKEY_ALREADY_REGISTERED)`. The message is
already fit to show a user, which is what [ADR-0008](../adr/0008-tray-and-toml-configuration.md)'s
"fail loudly on conflict" requires — task 5.4 of `add-manual-layout-conversion` can surface
this text directly rather than translating an error code.

**Confirmed cross-process.** A second instance of the spike, launched while the first held
the hotkey, failed identically at its *first* registration:

```
RegisterHotKey FAILED: Hot key is already registered. (0x80070581)
```

Same code, same message, across a process boundary. Detection does not depend on the
collision being same-process.

**`Pause` is unclaimed on this machine.** The first registration succeeded, which supports
its choice as the default binding in the configuration schema.

### Timing — answered: 500 ms is comfortable

Observed deltas, tapping deliberately at single, double and triple speeds:

| Delta (ms) | Ticks (÷15.625) | Intent |
|---|---|---|
| 141, 141 | 9.02 | deliberate multi-tap |
| 156, 156, 156, 157 | 9.98–10.05 | deliberate multi-tap |
| 2531, 2703, 3062 | 162–196 | deliberate new correction |

**Granularity is 15.625 ms** — every observed delta is a clean multiple of it. That is the
default Windows timer tick, and `MSG.time` inherits it. A 500 ms window is therefore about
32 distinguishable steps wide, which is far more resolution than the boundary needs.

**500 ms works, with a wide margin on both sides.** Deliberate multi-taps clustered at
141–157 ms — roughly a third of the window — and deliberate separations at 2531 ms and up,
five times the window. The nearest approach to the boundary from either direction was 157 ms
below and 2531 ms above. Nothing landed near 500 ms, so the default needs no adjustment;
[ADR-0009](../adr/0009-multi-tap-layout-cycling.md)'s proposed value is confirmed rather
than merely assumed.

A fourth tap in a run classified as `tap 4 in run`, as designed — this spike does not know
the candidate list, so it reports depth rather than wrapping. Wrapping is
`lexiswitch-core`'s job and is a pure function of press count.

---

## Consolidated `windows` crate feature set

All three spikes compile against `windows = "0.62"` with the union below. This is the
**floor** for `lexiswitch-win`, not its total — UI Automation, the clipboard, and the tray
icon are not exercised by any spike and will add features of their own.

| Feature | Needed for |
|---|---|
| `Win32_UI_Input_KeyboardAndMouse` | `GetKeyboardLayoutList`, `ToUnicodeEx`, `MapVirtualKeyExW`, `RegisterHotKey`, and `HKL` |
| `Win32_UI_Input` | `RegisterRawInputDevices`, `GetRawInputData`, `RAWINPUT`, `RIM_TYPEKEYBOARD` |
| `Win32_UI_WindowsAndMessaging` | message-only window, message loop, `WM_INPUT`, `WM_HOTKEY` |
| `Win32_System_LibraryLoader` | `GetModuleHandleW` for the window class instance |
| `Win32_System_Threading` | `OpenProcess`, `QueryFullProcessImageNameW` |
| `Win32_Globalization` | `LCIDToLocaleName` |
| `Win32_Graphics_Gdi` | **nothing drawn** — required only because `WNDCLASSW` carries `HBRUSH`/`HICON`/`HCURSOR`, putting the struct and `RegisterClassW` behind this feature |

Two placements contradict where the change's tasks expected them, and both cost a build
failure to discover:

- **`HKL` is in `Win32_UI_Input_KeyboardAndMouse`**, not `Win32_UI_TextServices`.
- **`RIM_TYPEKEYBOARD` is in `Win32_UI_Input`**, not `Win32_UI_WindowsAndMessaging` where
  the other `RIM_*`-adjacent message constants live.

One API shape worth carrying forward: windows-rs exposes `DefWindowProcW` as a plain Rust
`unsafe fn`, not `extern "system"`, so it cannot be assigned to `WNDCLASSW::lpfnWndProc`
directly. Both windowed spikes wrap it in a one-line `extern "system"` trampoline, and
`lexiswitch-win` will need the same.
