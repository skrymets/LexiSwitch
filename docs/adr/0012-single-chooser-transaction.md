# ADR-0012: One chooser transaction instead of multi-tap cycling

## Status

Accepted — supersedes [ADR-0009](0009-multi-tap-layout-cycling.md).

## Context

ADR-0009 made repeated presses of the same hotkey the mechanism for both choosing a target
layout and undoing a conversion, on the grounds that it was one rule rather than two features.
The rule was elegant on paper. In use it fails in four separate ways.

**It depends on state the user cannot see.** Which candidate a press produces depends on how
many presses preceded it and how recently, neither of which is visible. The rule's own
elegance is what hides it: there is nothing on screen to indicate a cycle is in progress.

**It depends on timing.** The multi-tap window makes the same physical action mean different
things depending on how fast it was performed, which is a race the user has to win.

**It depends on installed-layout order.** Candidates arrive in the order Windows reports
layouts, so choosing a target means remembering an ordering the user never set and cannot see.

**On a selection it cannot work at all.** After a replacement the text is no longer selected —
the caret sits at the end. A second press therefore finds no selection, falls through to the
clipboard read, sends `Ctrl+C` with nothing selected, and gets back the clipboard's existing
contents, which the read path cannot distinguish from a real selection because it is not
empty. The second press would then convert and paste unrelated clipboard content into the
document. This was identified by reading the code, not reproduced — the cycling path is being
removed rather than debugged, so confirming it would cost more than it is worth. It is
recorded because it explains why cycling was never going to work on selections, independently
of the three usability failures above.

## Decision

**One invocation opens one chooser, which shows every distinct candidate at once, and closes
when the user commits or cancels.** Candidate selection happens inside that transaction with
ordinary arrow keys.

Specifically: no repeated-press semantics anywhere in the product, no multi-tap window, no
candidate index carried between invocations, and no dependence on installed-layout order for
navigation. Every candidate is labelled with its own source and target layout, so choosing one
requires reading rather than remembering.

Every candidate is generated from the original selected text. ADR-0009 already required this —
rendering from the retained original rather than from the previous result — and it is the one
part of that decision worth carrying forward intact.

## Consequences

**Positive.** The hidden state disappears rather than being surfaced: there is no cycle
position to display because there is no cycle. Timing stops being part of the interaction.
Undo becomes the host application's `Ctrl+Z` acting on one edit, rather than a second press of
a hotkey, which means it works the way every other editing operation in the application works.

The `multi_tap_window_ms` setting and the tap-tracking state retire with the rule.

**Negative.** A conversion now costs at least two keystrokes — invoke, then commit — where the
best case under cycling was one. This is the deliberate trade the pivot makes: the brief
requires that text is never modified before an explicit commit, so a one-keystroke path cannot
exist without reintroducing the silent-modification problem.

The chooser is also substantially more to build than the cycling rule it replaces. That cost
is real and is the dominant cost of the pivot.

**Unresolved.** What makes two candidates the same. Deduplicating on visible result alone
collapses candidates whose targets differ — with en-US and de-DE installed, `uk→ENG` and
`uk→DEU` produce identical text for any selection that avoids the transposed keys and
differing punctuation — and the target is exactly what apply-and-switch acts on. Deduplicating
on the pair instead shows rows that look identical. This needs deciding before the chooser's
data model is fixed; it is noted here because ADR-0009's ordering rule is what previously made
the question moot.

## Alternatives Considered

**Keep cycling, but re-select the replaced text after each press.** Would repair the selection
failure, and nothing else. Rejected: the hidden state, the timing dependence and the
layout-order dependence are all independent of it, and they are the three the brief's
diagnosis actually names.

**Keep cycling as a fast path alongside the chooser.** Rejected: two mechanisms for one
operation, with the fast path being the one that modifies text without confirmation. The brief
explicitly rules out immediate modification for the redesigned baseline, and an optional
adaptive fast path is deferred rather than designed now.

**Show a candidate index such as `1 of 2` and keep repeated presses.** Rejected: it surfaces
the hidden state without removing the timing dependence or the need to press repeatedly to
discover what is available.

## Related

- [ADR-0009](0009-multi-tap-layout-cycling.md) — the cycling rule this replaces
- [ADR-0011](0011-no-keyboard-observation.md) — the observation removal this accompanies
- [ADR-0010](0010-paste-selection-replacement.md) — why replacement is one undo step, which is what makes the host's `Ctrl+Z` an adequate undo
- [Spike: focus round trip](../spikes/2026-08-07-focus-round-trip.md) — that a focused chooser can still replace the original selection
