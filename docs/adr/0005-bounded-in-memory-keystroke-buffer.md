# ADR-0005: Bounded in-memory keystroke buffer, with containment rules

## Status

Accepted

## Context

"Fix the last word I typed" requires knowing what was typed. That is the capability which
makes this class of utility a keylogger risk, and it cannot be designed around — only
contained.

One alternative genuinely avoids retaining anything: on hotkey, send `Ctrl+Shift+Left` to
select the previous word and convert the selection. No keystroke history exists in the
process at all. It fails, though, in applications with non-standard editing models,
depends on every target app honoring word-select consistently, and destroys the user's
existing selection on every invocation.

[ADR-0003](0003-raw-input-over-low-level-hook.md) establishes that LexiSwitch cannot
*intercept* keystrokes. It says nothing about what LexiSwitch *retains*. Since v1 is
manual-only ([ADR-0007](0007-manual-only-v1.md)), the buffer is the sole component that
holds data passively, between explicit user actions. Its containment rules are therefore
the entire runtime privacy model.

## Decision

Keep a **bounded 64-entry ring buffer** of recent keystrokes, in memory only.

**What it holds.** `(scancode, shift_state, tick)` — physical key positions, not
characters. Resolution to text happens only at conversion time, from a snapshot.

**Where it lives.** A fixed-size array owned outright by the message-loop thread. Never
shared, never behind a mutex; the worker thread receives a snapshot of the extracted word,
not a reference. Its pages are `VirtualLock`ed to keep them out of the pagefile, and
zeroized on every wipe and on drop.

**When it fills.** Everywhere *except*:

- the focused control is a password field — UI Automation `IsPassword`, falling back to
  the `ES_PASSWORD` window style;
- the foreground application matches the configured exclusion list;
- the user has paused LexiSwitch via hotkey or tray.

**When it is wiped.** Foreground-window change; `Enter`, `Tab`, or `Escape`; idle timeout
(default 30 s); any successful conversion; pause.

**What never happens.** Nothing derived from keystrokes is written to disk — not to
config, not to logs, not to a crash dump. Logging records event kinds and lengths
(`converted_word len=7`), never content, and is off by default.

## Consequences

**Positive.** Last-word correction works in every application, including terminals and
Electron apps where selection-based approaches are unreliable, and without disturbing the
user's selection. Sole ownership by one thread removes a class of concurrency bug from
the most sensitive data in the program. Storing scancodes rather than characters means a
memory disclosure yields key positions requiring a layout table to interpret — a modest
gain, but free.

**Negative, and this is the honest cost.** A buffer of recent keystrokes exists in
process memory. Nothing about the architecture makes misuse impossible the way ADR-0003
makes interception impossible; the guarantee here rests on the code being small, readable,
and audited. That is a weaker class of assurance, and it is the reason the buffer is
capped at 64 entries and aggressively invalidated — to bound the exposure rather than
argue it away.

**Password-field detection is good, not perfect.** It is reliable for native Win32 and
WinUI controls. Inside Chrome, Firefox, and Electron applications, whether the focused
element reports `IsPassword` depends on the app's accessibility implementation. This is a
known gap. It is mitigated by the exclusion list, the pause hotkey, the tray kill-switch,
and the short invalidation timeouts — and it must be measured with `probe_focus` across
the real application matrix *before* the buffer rules are trusted, not assumed.

## Alternatives Considered

**No buffer — select-and-convert.** Send `Ctrl+Shift+Left`, convert the selection. The
keylogger question becomes structurally unanswerable-wrongly, since no history exists.
Rejected: it clobbers the user's selection on every use, depends on target applications
honoring word-select, and fails in the terminals and editors where this tool is most
wanted. The privacy gain is real but the functionality loss is larger than the problem
being solved.

**Buffer with select-and-convert fallback.** Use the buffer when populated, fall back to
selection when empty or stale. Most robust behavior. Rejected for v1 as carrying two
mechanisms and two failure modes for a marginal gain; the empty-buffer case is a no-op
with a tray hint instead. Reconsider if the no-op proves annoying in practice.

**Allowlist-only buffering.** Fill the buffer solely in explicitly named applications, so
an undetected password field in an unlisted app is a non-issue. Strongest containment.
Rejected: it demands list curation up front and makes the tool silently inert in every
application the user forgot — which trains the user to distrust it.

## Related

- [ADR-0003](0003-raw-input-over-low-level-hook.md) — the capability ceiling this operates within
- [ADR-0007](0007-manual-only-v1.md) — why the buffer is the only passive component in v1
- [ADR-0002](0002-rust-as-implementation-language.md) — memory safety around this buffer
