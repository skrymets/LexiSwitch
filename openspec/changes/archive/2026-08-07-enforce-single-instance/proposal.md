# Enforce a single instance

## Why

Nothing prevents a second LexiSwitch running, and there is no use for one.

A second instance shares everything that matters and cooperates on none of it. Both read the
same configuration file and either may write it, so a shortcut recorded in one is silently
overwritten by the other. Both write the same log. Both put an icon in the tray, identical and
indistinguishable. And the second cannot register the shortcut, because the first already
holds it — so it comes up in the conflict state, reporting a conflict with itself and inviting
the user to resolve it by choosing a different shortcut, which is not the problem.

This is not hypothetical. Hours of this project's own development were lost to a stale instance
holding `Shift+Pause` while a freshly built one appeared to be broken, and to a tray icon that
turned out to belong to a process nobody remembered starting. The shortcut-conflict handling
built in `rework-shortcut-and-shell` was motivated in part by that experience — the machinery is
still right for a genuine conflict with another application, but a program conflicting with
itself is a defect, not a state to present nicely.

There is no counter-case. Per-user separation is already provided by Windows sessions; two users
each get their own agent regardless. Nothing about the product benefits from two agents in one
session.

## What Changes

- A second instance detects the first and does not start.
- It says so rather than exiting silently, and asks the running instance to make itself visible,
  so the user learns where the existing one is instead of concluding the program is broken.
- The detection is atomic, so two simultaneous starts cannot both conclude they are first.
- The shortcut-conflict state stays exactly as it is. It is now reachable only by a genuine
  conflict with another application, which is what it was always for.

## Capabilities

- `agent-control` — the shell's startup behaviour and what the user is told.

## Impact

- `lexiswitch-win` gains a single-instance guard; `lexiswitch` consults it before anything else.
- Three test harnesses create a shortcut conflict by starting a second LexiSwitch. They must
  instead use a separate program that holds the combination, which is a better test of a
  third-party conflict anyway.
- `docs/testing/manual-checklist.md` item B3 becomes unreachable as written and needs restating.
