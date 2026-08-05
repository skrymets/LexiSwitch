# ADR-0008: Tray icon and TOML file for configuration; GUI deferred

## Status

Accepted

## Context

LexiSwitch needs configuration for hotkey bindings, candidate layout order, the
application exclusion list, timeouts, and whether the clipboard fallback is permitted.

It runs as a background agent with no main window, so it needs a tray presence regardless
— for enable/pause, for surfacing errors, and so that a program observing keystrokes is
visibly present rather than hidden.

The question is what sits behind the tray. A settings GUI is friendlier, particularly for
interactive hotkey capture, where clicking a field and pressing the combination beats
writing `Ctrl+Shift+Pause` correctly by hand. But a UI toolkit is a dependency, binary
weight against a 5 MB budget, and a meaningful share of v1 effort spent on something other
than the actual problem.

There is also an argument specific to this project. Its premise is that the user should
not have to trust a keystroke-observing program on assurances alone. A plain-text config
is legible, diffable, greppable, and version-controllable; the user can see the entire
behavior of the program in one file. A GUI writes opaque state somewhere and shows a
rendering of it.

## Decision

**v1:** a tray icon plus a hand-edited TOML file.

- Tray menu: Enable / Pause, Reload config, Open config, Quit.
- Config at `%APPDATA%\LexiSwitch\config.toml`, with a documented `config.example.toml`
  in the repository.
- Hot-reload on change, detected by polling the file's mtime every 2 seconds.
- Parse errors keep the last-good configuration running and report the offending line via
  a tray balloon. LexiSwitch never runs unconfigured.

**Phase 4:** a settings GUI — interactive hotkey capture, exclusion-list editor, layout
pair configuration. Committed to the roadmap, not to v1.

## Consequences

**Positive.** Almost no UI code in v1. The entire configuration is one readable file the
user can inspect, diff, and back up — which matches the project's premise rather than
working against it. Hot-reload makes iterating on exclusion rules fast. Binary stays well
inside budget.

**Negative.** Hotkey binding is written by hand, so combinations must be spelled
correctly; the config parser needs genuinely useful error messages rather than a serde
default, since the error message is the only feedback mechanism. Editing a file is a
worse experience than a dialog for anyone who is not this user — a real cost, deferred
rather than denied, and acceptable while the tool has one user who writes Rust.

**mtime polling over `ReadDirectoryChangesW`.** Polling is a few lines and has no failure
modes worth discussing. `ReadDirectoryChangesW` is the correct API for watching a
directory of files changing rapidly; for one file edited by hand a few times a month, it
is a background thread, a buffer, and an overlapped-I/O lifetime to get right in exchange
for latency nobody perceives. A 2-second delay after saving is not a cost.

**A tray icon is not optional here.** Beyond mechanics, a program that observes keystrokes
should be visible while it runs. Hiding it would contradict the project's own argument.

## Alternatives Considered

**Tray plus native settings window in v1.** Friendlier, especially for hotkey capture.
Rejected for v1: a UI toolkit dependency and a substantial slice of effort spent away from
the conversion problem, for a tool with one technical user. Scheduled as Phase 4.

**TOML only, no tray.** Smallest possible. Rejected: nowhere to surface errors — and
config parse failures, hotkey registration conflicts, and clipboard timeouts all need to
reach the user — and no pause control, no kill switch, and no visible indication that a
keystroke-observing process is running.

## Related

- [ADR-0007](0007-manual-only-v1.md) — phase sequencing
- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — the exclusion list and pause control configured here
- [ADR-0006](0006-uia-first-clipboard-fallback.md) — the clipboard fallback toggle
