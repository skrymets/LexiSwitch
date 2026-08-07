## Context

See proposal.md for motivation. Three facts about the current tree shape the approach.

`focus.rs` splits cleanly: `Uia` shares nothing with `PasswordGate`, and exactly one product
line uses the gate. Capture touches roughly 30 of `main.rs`'s 511 lines, so removal is
surgical rather than a rewrite. And selection conversion already works, so the tree is
expected to be green and usable at every point during this change.

The constraint that shapes everything else is that `deny_unknown_fields` is set on every
config struct. A config file written by the current version will fail validation outright
against the new schema, and validation failure means the system keeps its last known good
configuration — which, on a first run after upgrade, does not exist.

## Goals / Non-Goals

**Goals**

- Leave a working selection-conversion tool at every commit, not only at the end.
- Prove the absence of observation capability mechanically, not by inspection of the source.
- Handle existing config files without a crash and without silently discarding intent.

**Non-Goals**

- The candidate chooser. This change removes; it does not replace.
- Fixing the endpoint-protection block. That is an experiment this change makes possible, not
  an outcome it promises.
- Reworking the shortcut conflict UX or the tray's state model beyond the pause/disable
  rename. Both belong to a later change.

## Decisions

**Remove in dependency order, leaf-first, keeping the tree green throughout.** Delete the
consumers before the things they consume: the last-word and clipboard hotkey arms in
`main.rs`, then the worker jobs they posted, then the buffer and cycling they used, then Raw
Input and page-locking. The alternative — deleting modules first and repairing the fallout —
produces a long red window and makes it hard to tell an intended removal from a mistake.

**Unknown config keys are ignored with a report, not rejected.** Three options were weighed.
Rejecting is the current behavior and is wrong here: it turns an upgrade into a failure to
start. Silently ignoring is friendlier but loses the distinction between a setting that was
removed and one that was misspelled, which is exactly the distinction a user needs when a
setting appears to have no effect. Rewriting the user's file in place was rejected outright:
the file is the user's, and a program that edits it is a program the user can no longer reason
about with a diff.

So: relax `deny_unknown_fields` on the top-level config, collect unrecognised keys, and report
them by name at startup. A misspelled key is then visible in the same report as a removed one,
which is a better outcome than the current silence-or-failure pair.

**Absence is proven against the release binary, not the source.** Grepping the source for
`SetWindowsHookEx` proves nothing about what a dependency does. The check inspects the built
release binary for `RegisterRawInputDevices`, `RIDEV` and `SetWindowsHookEx`, which is the
same standard ADR-0003 set for auditors reading the imports.

**`SYNTHETIC_MARKER` goes with Raw Input.** The marker exists so this program's own injected
input could be recognised and discarded by its own observation. With nothing observing, the
marker tags input that nothing reads. Keeping it would leave a reader wondering what consumes
it.

## Risks / Trade-offs

**Removing `deny_unknown_fields` weakens typo detection** → The startup report replaces it,
and reports strictly more than the current behavior did: today a typo inside a known section
fails the whole load with a parse error, giving no partial function. Named, non-fatal
reporting is the better trade for a tool that must keep running.

**The product loses a feature before its replacement exists** → Deliberate, and the reason
this change is separable. Between this change and the chooser, selection conversion works and
is the only conversion; that is a coherent product, not a broken one.

**Undo becomes the host's `Ctrl+Z` with no cycling to fall back on** → Acceptable because
replacement is already one logical edit ([ADR-0010](../../../docs/adr/0010-paste-selection-replacement.md)),
so a single `Ctrl+Z` reverts a whole conversion. This was measured, not assumed.

**The endpoint-protection experiment may return an unwelcome answer** → If the release build
is still quarantined after capture is gone, the pivot's §22 criterion is unmeetable by this
work and the cause lies elsewhere, most plausibly in signing and binary reputation. That is
worth knowing early, which is why the re-test is part of this change rather than a later one.

## Migration Plan

1. Land the removals with the tree green at each step.
2. On first run against an old config file, the system starts and reports the ignored keys.
3. No rollback path is needed for user data: the config file is never written by the program,
   so reverting the binary restores the previous behavior exactly.

## Open Questions

**Does the release binary stop being quarantined?** Deferrable because the answer changes
neither the specs nor the approach — only what can be claimed afterwards. It is recorded as a
task rather than an assumption.


## Appendix: the retired `keystroke-capture` capability

The capability is deleted outright rather than edited down — with keystroke observation gone
([ADR-0011](../../../docs/adr/0011-no-keyboard-observation.md)) there is nothing left for it to
describe, and a capability with no requirements is not a valid spec.

It is recorded here rather than as a spec delta because the tooling cannot express deleting a
capability: a delta that removes every requirement rebuilds the spec as empty and is rejected,
and with the spec already deleted the same delta is treated as creating a new empty one. The
main spec directory `openspec/specs/keystroke-capture/` was therefore removed directly, as part
of this change.

The requirements it held, all now void:

- Observation cannot block or alter input
- The buffer is bounded and memory-resident
- Nothing derived from keystrokes reaches disk
- The buffer does not fill in unsafe contexts
- The buffer is wiped at word and context boundaries
- The buffer records key positions, not characters
