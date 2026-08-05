# ADR-0003: Raw Input observation instead of a low-level keyboard hook

## Status

Accepted

## Context

This is the decision the project exists to make.

LexiSwitch was written because Punto Switcher — a Yandex product from a
russian-controlled company — installs a global keystroke interceptor. The objection is not
that Punto is known to exfiltrate keystrokes; it is that a `WH_KEYBOARD_LL` hook *can*,
and that the vendor is not one whose assurances carry weight. Rebuilding the same
mechanism and asking for trust on better grounds reproduces the problem rather than
solving it.

Windows offers two ways to observe keystrokes globally from an unprivileged process:

- **`SetWindowsHookEx(WH_KEYBOARD_LL)`** — a callback in the system input path. It
  receives each keystroke *before* the target application and can return `1` to discard
  it. This is the mechanism keyloggers use, and EDR products score it accordingly.
- **Raw Input with `RIDEV_INPUTSINK`** — a hidden message window receives `WM_INPUT` for
  keyboard events regardless of focus. The API delivers a *copy* of the event after the
  fact. It has no return value that affects delivery. Blocking or altering a keystroke is
  not something the caller declines to do; it is not expressible.

Two properties of the low-level hook matter beyond capability. It runs inside the
system-wide input path, so a callback exceeding `LowLevelHooksTimeout` (~300 ms default)
gets the hook silently evicted, and typing behaves strangely everywhere until restart.
And it is a first-tier heuristic signature for Defender and EDR products.

A capability the design never needs: correction works retroactively, by sending
backspaces and retyping. No keystroke ever needs to be suppressed before the focused
application sees it. The hotkey itself is consumed by `RegisterHotKey`, which does not
require a hook.

## Decision

Observe keystrokes exclusively through **Raw Input** (`RIDEV_INPUTSINK`) on a hidden
message window. Trigger actions through **`RegisterHotKey`** / `WM_HOTKEY`.

**`SetWindowsHookEx` is not called anywhere in LexiSwitch, and no abstraction is provided
that would make adding it easy.**

## Consequences

**Positive.** The security claim becomes structural: LexiSwitch cannot intercept
keystrokes, because the API it uses has no such capability. That is verifiable by reading
the imports, not by auditing behavior. No `LowLevelHooksTimeout` hazard, so a slow UI
Automation or clipboard call can never degrade system-wide typing. Lower EDR heuristic
weight — `RIDEV_INPUTSINK` is the pattern games use for input devices. Raw Input delivers
scancodes natively, which is exactly the physical-key data the conversion needs, rather
than characters that must be reverse-mapped.

**Negative.** Keystrokes cannot be suppressed before delivery. This costs nothing for the
planned feature set, but it forecloses any future feature that would need it — for
example, swallowing a mistyped character rather than backspacing over it. If such a
feature is ever wanted, it requires a new ADR superseding this one, which is the intended
friction. `RegisterHotKey` also fails if another application already holds the
combination; the design treats this as a loud startup error rather than a silent
degradation.

**Neutral, and worth stating plainly.** Raw Input still observes keystrokes in every
application. This ADR establishes a capability ceiling, not privacy on its own. Privacy
comes from the buffer containment rules in [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md)
and from the no-network, no-telemetry constraints in the spec.

**Open, to be settled by spike.** Whether `RIDEV_INPUTSINK` is subject to the same UIPI
restriction as hooks — that is, whether an unelevated LexiSwitch sees keystrokes typed
into elevated windows. Low-level hooks do not. If Raw Input behaves the same, it is an
accepted and documented limitation, not a design change.

## Alternatives Considered

**`WH_KEYBOARD_LL` low-level hook.** Universal, thoroughly documented, and the approach
`rdev` and Punto both take. Rejected: it holds precisely the capability this project
rejects, adds the input-path stall hazard, and draws the heaviest EDR scrutiny — while
its one unique power, keystroke suppression, is never needed here.

**Two-process privilege separation.** A minimal observer process with no filesystem or
network dependencies, piping scancodes over a named pipe to a logic process. The audit
boundary sounds strongest. Rejected: the buffer, the conversion, and the injection all
live in the logic process regardless, so the isolation is largely presentational, while
the IPC, dual lifecycle, and orphan-process handling are entirely real. Worth revisiting
only if LexiSwitch is ever distributed to people who cannot read the source.

**Raw Input behind a trait, with the hook as a swappable second implementation.** Would
de-risk the UIPI spike. Rejected deliberately: an abstraction that makes the hook a
configuration change undermines the property this ADR establishes. If the spike forces a
reversal, that warrants a superseding ADR and an honest change to the project's claims —
not a feature flag.

## Related

- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — the containment rules that supply actual privacy
- [ADR-0009](0009-multi-tap-layout-cycling.md) — why `RegisterHotKey` timing suffices
- [Change: spike-windows-input-primitives](../../openspec/changes/spike-windows-input-primitives/proposal.md) — the UIPI spike that settles the open question above
