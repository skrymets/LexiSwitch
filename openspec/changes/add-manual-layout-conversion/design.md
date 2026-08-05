## Context

See `proposal.md` — Why. Requirements are in `specs/`; this document covers how they are
met.

Nothing exists yet beyond decision records and the spike change. The constraints that
shape the structure: a 5 MB binary and 20 MB resident budget, all logic must be testable
without a Windows machine or a keyboard in the loop, and every `unsafe` block must be
individually auditable — the security argument rests on a reader being able to check it.

The nine decisions this implements are recorded in `docs/adr/`, with their rejected
alternatives. This document does not re-argue them.

## Goals / Non-Goals

**Goals:**

- Conversion logic that runs and is tested with zero Win32 present.
- `unsafe` confined to one crate, each block justified in place.
- No component able to stall system-wide typing, regardless of how slow it is.
- The whole configurable surface visible in one readable file.

**Non-Goals:**

- A platform abstraction validated against more than one platform. The core/platform
  boundary exists for testability ([ADR-0001](../../../docs/adr/0001-windows-first-with-os-free-core.md));
  an interface designed against a single implementation is a guess, and the first real
  second platform will revise it.
- Reusable library APIs. Nothing here is consumed by anything but this binary.
- Performance work beyond meeting the stated budgets.

## Decisions

**Three crates, split by what they are allowed to call.**
`lexiswitch-core` (no Win32, `#![forbid(unsafe_code)]`, all logic), `lexiswitch-win` (all
FFI, all `unsafe`), `lexiswitch` (wiring). Per
[ADR-0001](../../../docs/adr/0001-windows-first-with-os-free-core.md). The boundary is
enforced by the dependency graph rather than convention: core cannot call Win32 because it
does not depend on anything that can.

**Two threads, with the buffer owned outright by one of them.**
The message-loop thread runs the window, Raw Input, hotkeys, and tray. It never blocks —
UI Automation and clipboard calls can stall for seconds, and the message loop is also what
keeps the tray responsive. It owns the keystroke buffer exclusively. On a hotkey it
extracts a snapshot of the relevant text and sends that over a channel to a worker thread,
which performs the automation, clipboard, and input-synthesis work.

The buffer is therefore never shared and needs no lock. Alternative considered: a
mutex-guarded buffer both threads reach. Rejected — it makes the most sensitive data in
the program concurrent for no gain, since the worker needs a point-in-time snapshot
anyway, not live access.

**Layout tables built once at startup, then immutable.**
Per [ADR-0004](../../../docs/adr/0004-runtime-derived-layout-tables.md). Rebuilding on each
conversion would mean calling into layout probing on the hot path and re-encountering
the dead-key state hazard repeatedly. A layout installed after startup requires a restart;
that is an acceptable cost for a table that is otherwise read-only and shareable across
threads without synchronization.

**Word extraction resolves at conversion time, not capture time.**
The buffer holds key positions ([ADR-0005](../../../docs/adr/0005-bounded-in-memory-keystroke-buffer.md)).
Deciding where a word begins requires interpreting those positions as characters, which
requires a layout table — so extraction happens when a hotkey fires, against the layout
that was active while typing. This keeps the capture path trivial, which matters because
it runs on the message-loop thread.

**Selection replacement tries automation first, clipboard second.**
Per [ADR-0006](../../../docs/adr/0006-uia-first-clipboard-fallback.md). Detection is by
attempt, not by maintaining a list of known-good applications: try the automation path,
fall back on failure. An application list would be permanently out of date and would fail
in exactly the direction that surprises the user.

**Clipboard restoration covers text formats only.**
Other formats are left untouched rather than snapshotted and rewritten. A partial
reconstruction of rich or image content that looks successful is worse than an honest
text-only restore, because the user has no reason to check.

**Errors surface through the tray, and every failure path has one.**
There is no console and no log by default, so an unreported failure is invisible. The
`agent-control` spec requires that notifications state what did not happen — "clipboard
busy, selection not converted", not "error 5".

**Configuration is parsed into validated types once, then treated as immutable.**
A reload builds a new configuration and swaps it wholesale; nothing reads half-old,
half-new state. Validation failure means the swap does not happen, which is how the
last-known-good requirement is met without a rollback path.

## Component layout

`lexiswitch-core` — no Win32, all tested without Windows:

| Module | Responsibility |
|---|---|
| `layout.rs` | `LayoutTable`: `(scancode, shift_state) → char` and the reverse map. Built from plain data, so tests load JSON fixtures. Dead-key positions marked and excluded from conversion. |
| `convert.rs` | `convert(text, from, to) -> Option<String>`. Character → physical key → character in target. `None` when nothing would change. |
| `buffer.rs` | `KeyBuffer`: 64-entry ring of `(scancode, shift_state, tick)`, word extraction, invalidation rules. |
| `cycle.rs` | Candidate selection and multi-tap cycling. |
| `config.rs` | Config types, defaults, validation with line-referenced errors. |
| `exclusion.rs` | Process-name and window-class matching. |

`lexiswitch-win` — all FFI, all `unsafe`:

| Module | Responsibility |
|---|---|
| `rawinput.rs` | Hidden message window, `RIDEV_INPUTSINK`, `WM_INPUT` parsing. |
| `hotkey.rs` | `RegisterHotKey` / `WM_HOTKEY`, multi-tap classification from message timestamps. |
| `layouts.rs` | `GetKeyboardLayoutList` + `ToUnicodeEx` probing, dead-key state flushed between probes. |
| `focus.rs` | Foreground window → process name; password-field probe. |
| `inject.rs` | `SendInput`: backspaces then `KEYEVENTF_UNICODE`. |
| `selection.rs` | Automation text pattern read/replace; delegates to `clipboard.rs` on failure. |
| `clipboard.rs` | Snapshot and restore text formats; other formats untouched. |
| `langswitch.rs` | Layout activation via `WM_INPUTLANGCHANGEREQUEST`. |
| `tray.rs` | Tray icon, menu, notifications. |

`lexiswitch` — `main.rs`, `app.rs` (threading and wiring), `configwatch.rs` (mtime poll).

## Configuration schema

`%APPDATA%\LexiSwitch\config.toml`, documented by `config.example.toml`:

```toml
[hotkeys]
convert_last_word = "Pause"
convert_selection = "Shift+Pause"
convert_clipboard = "Ctrl+Pause"
pause_toggle      = "Ctrl+Shift+Pause"
multi_tap_window_ms = 500

[layouts]
# Cycle order. Empty means installed order, layout active at typing time first.
candidates = []

[buffer]
idle_timeout_secs = 30
respect_password_fields = true   # false is refused by validation

[selection]
allow_clipboard_fallback = true  # false restricts conversion to automation-capable apps

[exclusions]
processes = ["KeePassXC.exe", "1Password.exe"]

[behavior]
switch_layout_after_correction = true

[logging]
enabled = false   # event kinds and lengths only, never content
```

`Pause` is chosen for the defaults because it is unbound on Windows and rarely claimed.
Keyboards without the key require rebinding, which the config supports.
`respect_password_fields = false` is rejected by validation rather than honoured; the
option exists in the schema so its presence is explicit rather than an oversight.

## Risks / Trade-offs

**Password-field detection is weaker inside browsers and Electron applications than in
native controls.** → This is the largest gap in the privacy model. `probe_focus` measures
actual coverage across the real application matrix *before* the buffer rules are trusted,
and the measured result is documented rather than assumed. Residual exposure is bounded by
the 64-entry cap, the exclusion list, the pause control, and short invalidation timeouts —
not argued away.

**The clipboard fallback exposes text to clipboard managers and Windows Clipboard
History.** → Inherent to the mechanism and identical to the user pressing `Ctrl+C`
themselves. Documented in [ADR-0006](../../../docs/adr/0006-uia-first-clipboard-fallback.md),
disableable in configuration, and the automation path is always attempted first so it is
not the common case.

**Synthetic input can race the target application.** → Sending backspaces and replacement
characters assumes the application processes them in order and that focus does not move
mid-sequence. A focus change during injection could deliver characters to the wrong window.
Mitigation: verify the foreground window is unchanged immediately before injecting, and
keep sequences short. This cannot be fully eliminated with synthetic input and is accepted.

**COM lifetime management for UI Automation is where Rust's guarantees stop.** → It is one
of the few places in the codebase where correctness depends entirely on getting `unsafe`
right. Mitigation: confine COM interaction to one module, wrap interfaces in RAII types at
the boundary so the rest of the crate never handles raw pointers, and never let a COM
interface cross the channel between threads.

**Backspace-based replacement is destructive if the word length is miscounted.** → Deleting
too many characters removes text the user did not type in the mistyped word. Mitigation:
count from the buffer's own record rather than inferring from the document, and decline to
act when the buffer is stale rather than guessing — the `manual-correction` spec requires a
no-op in that case.

**Budget verification is deferred to the end.** → Binary size and memory are only measured
once everything is present, so an over-budget result arrives late. Accepted: dependency
count is the dominant factor, it is reviewed as each dependency is added, and there are no
large dependencies planned.

## Open Questions

- The exact `windows` crate feature set is settled by `spike-windows-input-primitives`
  rather than guessed here. It affects compile time and binary size but not the approach
  or the task breakdown.
- Whether a 30-second idle timeout for buffer invalidation is comfortable in practice, or
  irritating. It is a configurable default, adjustable after use.
