# ADR-0015: Usage memory lives outside the configuration file

## Status

Accepted. Supersedes nothing. Refines the scope of two standing constraints —
[ADR-0008](0008-tray-and-toml-configuration.md)'s TOML configuration file, and the rule that
nothing derived from keystrokes reaches disk — by naming a second file and saying exactly
what may go in it.

## Context

The case window ([ADR-0014](0014-case-conversion-is-a-separate-transaction.md)) offers
fourteen styles in a fixed list and promotes the three most used to the top. That promotion
is worth having only if it survives a restart. The agent restarts on every reboot, so a
memory that lives only in process is relearned from scratch roughly weekly, which is close
enough to no memory at all that the feature would not be worth building.

So something has to reach disk, and two existing constraints bear on what.

**Nothing derived from keystrokes reaches disk.** This is one of the reasons the project
exists, and it is deliberately absolute — not config, not logs, not crash dumps. Logging
records event kinds and lengths (`converted_word len=7`), never content.

**The configuration file is the user's.** `rewrite_setting` exists specifically so that
writing one setting does not destroy the user's comments, spacing or ordering; it is a
surgical text edit rather than a parse-and-reserialise round trip, because the file is
hand-authored and the program is a guest in it. Two settings are already written back this
way — `general.enabled` and `hotkeys.convert_selection` — each in response to a deliberate
user action through the interface.

Usage counts are not like those two. They change on **every apply**, not when the user
decides something. Writing them into the configuration file would mean rewriting the user's
hand-authored document several times a day as a side effect of ordinary use, churning it
against any editor that has it open, and filling `git diff` — for those who keep their
dotfiles in version control — with counters.

## Decision

**Usage counts live in a separate state file, `%APPDATA%\LexiSwitch\state.toml`, beside the
configuration file and never inside it.**

What it may contain is closed, not open-ended:

- the **identifiers of case styles** — `upper`, `snake_case`, `words_capitalized` — which
  are this program's own vocabulary, fixed at compile time; and
- **integer counts** of how many times each was applied.

It may not contain selected text, any candidate, any preview, any process or window name, or
any timestamp finer than not-at-all. Nothing in it is derived from keystrokes: a style
identifier records which row of a menu the user chose, which is the same category of fact as
`general.enabled` recording that they clicked Disable. The no-content rule is about text the
user typed, and no text the user typed can reach this file.

The file is owned by the worker thread outright — loaded once, mutated in memory, written
after a commit or a reset — so it is never shared across threads and never needs a lock. It
is not watched and not hot-reloaded.

A missing, unreadable or malformed state file is **not an error**. It means "no memory yet":
the list falls back to its declared order and the file is rewritten on the next apply. There
is nothing in it worth failing to start over.

`Ctrl+R` inside the case window clears it, in place, without closing the transaction — so a
memory that has learned the wrong thing can be discarded at the moment the user notices it
has, rather than by finding and deleting a file.

## Consequences

**Positive.** The configuration file stays hand-authored and stays a complete description of
*behaviour*; it is not polluted with counters that describe *history*. The distinction is
worth keeping legible: one file the user writes, one file the program writes.

Because the format is closed to style identifiers and integers, the privacy claim is
checkable by opening the file, not by trusting a description of it. That is the standard the
rest of the project holds itself to.

**Negative.** A second file to document, to create, to locate when `APPDATA` is unset, and
to mention in any uninstall instructions. "Delete the configuration file to reset
everything" is no longer true, which is a small trap for a user who has read the old advice.

It is the first time the program writes anything that is not configuration, and that is a
line worth being conscious of having crossed. The mitigation is that the line is drawn
explicitly above rather than left to the next feature's judgement.

**Neutral.** The counts are unbounded integers. Overflow is not a practical concern at human
apply rates, and saturating arithmetic costs nothing, so it is used rather than reasoned
about.

## Alternatives Considered

**Keep the counts in the configuration file.** One file, and "the file is the complete
description of behaviour" stays literally true. Rejected: it rewrites the user's document on
every apply, fights the surgical-edit design that exists to protect it, and churns against an
open editor. Behaviour and history are different things and do not belong in the same file.

**Keep them in memory only.** Nothing new touches disk, which is the strongest reading of the
privacy constraint. Rejected: the agent restarts on every reboot, so the ordering is relearned
roughly weekly and the feature does not earn its keep.

**The registry.** Conventional on Windows for this kind of state. Rejected: the project keeps
everything it owns in one directory under `APPDATA`, which makes both backup and removal
obvious, and a registry key is neither inspectable by the user in the way a TOML file is nor
consistent with how the rest of the program is configured.

**Recency instead of frequency, needing only an ordered list of the last three.** A smaller
thing to store, and arguably the same benefit. Rejected on interaction grounds rather than
storage: a recency list reorders after every single use, so the row a user is reaching for
moves under their hand, which defeats the muscle memory the promotion exists to build.
Frequency with a fixed order during a transaction was chosen deliberately.

## Related

- [ADR-0014](0014-case-conversion-is-a-separate-transaction.md) — the window this memory serves
- [ADR-0008](0008-tray-and-toml-configuration.md) — the configuration file this one sits beside
- [ADR-0011](0011-no-keyboard-observation.md) — why nothing derived from typing exists to be written in the first place
