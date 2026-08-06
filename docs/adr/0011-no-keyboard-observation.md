# ADR-0011: No keyboard observation; operate only on an explicit selection

## Status

Accepted — supersedes [ADR-0003](0003-raw-input-over-low-level-hook.md) and
[ADR-0005](0005-bounded-in-memory-keystroke-buffer.md).

ADR-0003's *decision* — observe via Raw Input rather than a hook — retires with observation
itself. Its *prohibition* on `SetWindowsHookEx` does not retire; it is carried forward below
and strengthened, and it now constrains the user interface as well as the privacy model.

## Context

ADR-0003 and ADR-0005 exist to serve one feature: correcting the last word typed. That
requires knowing what was typed, which requires observing typing, which requires a buffer,
which requires everything built to contain the buffer — password-field detection, an
excluded-application list, a pause state, wiping on focus change and on `Enter`/`Tab`/`Escape`,
an idle timeout, page-locking, and zeroization.

The product pivot retires last-word correction. Nothing else in the product reads the buffer:
selection conversion reads the selection, and clipboard conversion reads the clipboard. The
entire observation subsystem becomes machinery with no consumer.

**The endpoint-protection question is unresolved and this ADR does not settle it.** A release
build is blocked and quarantined on the development machine by CrowdStrike Falcon, with a
generic verdict — "malicious behavior was detected" — and no attribution. The block may or may
not relate to Raw Input; every other suspicious attribute of that binary (unsigned, zero
prevalence, optimised, symbols stripped, no console, registering global hotkeys, calling
`SendInput`) survives this decision untouched. Removing observation is justified by the
product argument alone. Treating it as an EDR fix would be assuming a cause the evidence does
not name. See `docs/manual-tests.md`.

## Decision

**No keyboard observation of any kind.** Raw Input registration is removed along with the
buffer and everything that existed to contain it. The program receives keystrokes only as an
ordinary focused window receives them, and only while a window it owns has focus.

The prohibition ADR-0003 established is retained and widened: no `SetWindowsHookEx`, and no
mechanism that observes keystrokes outside a window this program owns and holds focus for. No
abstraction may be added that would make either easy to slot in later.

**The program acts only on text the user has explicitly selected, and only when explicitly
invoked.** There is no other input to a conversion.

### Why the prohibition now constrains the interface

This is the part ADR-0003 could not have anticipated, and it is why the prohibition is
load-bearing rather than merely principled.

The pivot's candidate chooser is keyboard-driven. A window that reads the keyboard must be
focused — and the alternative that avoids taking focus, a `WS_EX_NOACTIVATE` window fed by a
low-level keyboard hook, is exactly what is forbidden here. Registering `Up`, `Down`, `Enter`
and `Esc` as global hotkeys is not a serious substitute.

So the chooser must take focus, which means the target application's selection has to survive
losing and regaining it. That was measured before this decision was recorded rather than
assumed: it survives, across a Win32 control with a text pattern, a Scintilla control without
one, and a Chromium text field. See
[the focus round-trip findings](../spikes/2026-08-07-focus-round-trip.md).

A future proposal to relax the hook prohibition would therefore not be trading away privacy
alone. It would be trading away the reason the interface has the shape it has.

## Consequences

**Positive.** The observation subsystem and its containment machinery leave the tree
entirely — Raw Input registration, the ring buffer, page-locking, wiping rules, idle timeout,
password-field detection, excluded-application matching, pause state, and their configuration
and tests. Roughly 660 lines of source plus four test files, and a comparable amount of
reasoning that no longer has to be maintained or audited.

The privacy argument gets simpler in kind, not just in degree. "Keystrokes are captured but
contained" required trusting the containment rules; "keystrokes are not captured" does not.
`respect_password_fields` and the exclusion list stop being safety features and become
unnecessary, which is a better outcome than making them configurable.

**Negative.** Last-word correction is gone permanently. It cannot return without returning
observation, and this ADR is what would have to be superseded to bring it back. That is
deliberate: the pivot's diagnosis is that the feature's cost was never repaid.

**Unresolved.** Whether the release build stops being blocked. That is an experiment to run
once capture is removed, not a consequence to claim here.

## Alternatives Considered

**Keep capture, fix only the interaction problems.** The brief's diagnosis lists twelve
causes; most trace to observation or to the last-word feature it serves. Rejected: it would
retain the largest and least auditable subsystem to support the feature the pivot exists to
remove.

**Keep Raw Input, but register it only for the duration of a transaction.** Attractive as a
narrowing. Rejected: the capability would still be present in the binary, and one of the
pivot's user-facing invariants is that a release build contains no active general
keystroke-monitoring subsystem. A capability that is merely dormant does not satisfy a reader
auditing the imports, which was ADR-0003's own standard of proof.

**Retain the buffer for a future automatic mode.** Rejected: speculative retention of the
riskiest component, for a feature ADR-0007 already deferred.

## Related

- [ADR-0003](0003-raw-input-over-low-level-hook.md) — the hook prohibition carried forward here
- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — the buffer this removes
- [ADR-0007](0007-manual-only-v1.md) — manual-only operation, strengthened rather than changed
- [ADR-0012](0012-single-chooser-transaction.md) — the cycling rule that retires alongside this
- [Spike: focus round trip](../spikes/2026-08-07-focus-round-trip.md) — why a focused chooser is workable, and why the hook prohibition now shapes the interface
- `docs/manual-tests.md` — the endpoint-protection observation and its correction
