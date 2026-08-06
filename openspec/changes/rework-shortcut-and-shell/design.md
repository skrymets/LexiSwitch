## Context

See proposal.md for motivation. The machinery this change needs mostly exists.

`Hotkeys::register` returns a `Result` and names the failing combination; `Drop` unregisters;
configuration reload already tears down and re-registers, so runtime re-registration is not
only possible but exercised today. What is missing is the startup policy, which propagates the
failure and exits, and an interface for recording a combination.

The one genuinely new piece is a window that reads a key combination without acting on it —
the second small window in the product after the chooser.

## Goals / Non-Goals

**Goals**

- No startup condition ends the process without first telling the user what to do about it.
- Every state the shell can be in is visible from the tray.
- The verification debt on the shell is paid down, not carried forward.

**Non-Goals**

- Per-application shortcuts, dynamic reassignment, modifier double-taps, long-press and
  leader-key sequences. All are excluded by the pivot, and the last three would require
  keyboard observation.
- Suggesting conflict-free combinations as though they were guaranteed. Suggestions may be
  offered; none can be promised.
- The chooser. This change assumes it, or the existing conversion path, is what the invocation
  command triggers.

## Decisions

**Startup failures become states, not exits.** Both the shortcut conflict and the too-few-layouts
condition already have accurate error values; what changes is where they are delivered.
Instead of `?` at startup, they set a state the tray renders and the settings interface can
act on. This is a smaller change than it appears and removes the product's two unrecoverable
conditions.

**The tray gains a conversion command, and it is not a convenience.** It is what keeps the
product usable while the shortcut is unregistrable — otherwise a conflict leaves the user with
an application that runs and cannot do anything. The pivot requires that no *required* workflow
step depends on the mouse; this is a fallback path, not the required one, so a tray command
satisfies both.

**Shortcut recording captures keys through a focused window, never a hook.** The window has
focus while recording, so it receives key events as any focused window does. This is the same
constraint that shapes the chooser and comes from the same place
([ADR-0011](../../../docs/adr/0011-no-keyboard-observation.md)): the alternative — observing
combinations globally to detect what the user pressed — is precisely what may not be built.

**A failed rebind restores the previous combination rather than leaving nothing bound.** Silence
would be the worst outcome of an attempt to fix silence.

**Recording persists to the configuration file.** The file remains the complete description of
behavior, so a shortcut changed through the interface must be written there — otherwise the
file stops being the source of truth the specification requires. The program writes the file
only in response to an explicit user action, which keeps a diff meaningful.

## Risks / Trade-offs

**Writing the configuration file conflicts with "the user edits it directly"** → Confined to
explicit user actions and to the settings the interface exposes. Comments and formatting
elsewhere in the file must survive, or the file stops being something a user can maintain by
hand.

**A second small window doubles the hand-written Win32 surface** → Mitigated by sequencing:
build it after the chooser and reuse whatever that establishes. If this change lands first,
expect the chooser to reshape it.

**Live re-registration has a window where no shortcut is bound** → Unregister and re-register
in one operation, and treat failure as "keep the old one" rather than "leave it unbound".

**Detecting that a second layout was installed while running** → Requires either polling or a
system notification. Deferred to implementation, since the specification only requires that a
restart is not needed.

## Migration Plan

1. Land after `retire-keystroke-capture`, whose config reduction this builds on.
2. A user upgrading with a conflicting shortcut sees the conflict state instead of a process
   that fails to start — strictly better than the current behavior in every case.
3. No data migration. The configuration file gains no required keys.

## Open Questions

**How a second-layout installation is detected while running.** Deferrable: the specification
requires only that no restart is needed, and the choice between polling and a notification
changes neither the specs nor the task breakdown.
