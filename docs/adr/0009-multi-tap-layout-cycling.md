# ADR-0009: Multi-tap cycling as target selection and undo

## Status

Superseded by [ADR-0012](0012-single-chooser-transaction.md)

One part survives: every candidate is rendered from the retained original text rather than
from the previous result. ADR-0012 carries that forward. The repeated-press mechanism does not.

## Context

Two requirements looked separate and turned out to be the same requirement.

**Ambiguity with three or more layouts.** Layout tables are derived from whatever Windows
has installed ([ADR-0004](0004-runtime-derived-layout-tables.md)). With Ukrainian, English,
and German present, "convert this word" has no single answer — the text could be intended
as any of the other two. Guessing requires the language detection that v1 explicitly
defers ([ADR-0007](0007-manual-only-v1.md)).

**Undo.** The original brief asked for double-tapping the hotkey to reverse a conversion,
for cases where a technical term or password was deliberately typed in the other layout.

The obvious implementation treats these separately: some rule picks a target, and a
special-cased double-tap reverses whatever the last conversion did. That means conversion
history, a state machine for "was the previous action a conversion", and undo behavior
that differs depending on how the target was chosen.

## Decision

**Repeated presses of the same hotkey within the multi-tap window cycle the text through
the configured candidate layouts, wrapping back to the original.**

The nth press within the window renders the same source text under the nth candidate
layout. After the last candidate, the cycle returns to the original text.

Candidate order comes from config; it defaults to the installed layout order with the
layout active at typing time first. The multi-tap window defaults to 500 ms.

Consequences of the general rule:

- Two layouts installed → the second press restores the original. This *is* the
  double-tap undo from the brief; it is not implemented separately.
- Three layouts → press once for the second candidate, twice for the third, three times
  to return to the original.

Each press re-renders from the retained source text, so the operation is idempotent per
press count rather than a sequence of transformations applied to transformed output.

## Consequences

**Positive.** One rule covers target selection, disambiguation, and undo. No conversion
history, no undo state machine, no special case. It degrades gracefully: with two layouts
the behavior is exactly what the brief asked for, and with more it extends naturally.
Re-rendering from source each time means no drift from repeatedly converting already-converted
text. And it is fully testable in `lexiswitch-core` with no Win32 present — cycle logic is
a pure function of source text, candidate list, and press count.

**Negative.** With three or more layouts, reaching the third candidate takes two presses
within 500 ms, which is a learned motion. Users who want a specific target directly would
need dedicated per-layout hotkeys — worth adding if it becomes an irritation, but not in
v1.

**The multi-tap window is a real timing constraint.** Too short and deliberate double-taps
are missed; too long and the next single press is misread as a continuation. 500 ms is a
starting default to be tuned in use, and it is configurable for that reason. Classification
uses the timestamps carried on `WM_HOTKEY` messages rather than wall-clock reads inside the
handler, so that scheduling delay in the message loop cannot corrupt the count. That this
works as expected is a Phase 0 spike.

**Source text must be retained across the window.** For the duration of the multi-tap
window the pre-conversion text is held so each press can re-render from it. This is
bounded by the window duration and discarded when it closes — a short-lived extension of
the retention discussed in [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md), subject
to the same zeroize-on-discard handling.

## Alternatives Considered

**Configured layout pairs with an explicit undo action.** Each hotkey bound to a fixed
source→target pair, undo as its own binding. Predictable and direct. Rejected: it needs a
hotkey per pair — six bindings for three layouts — plus conversion history for undo, and
the number of bindings grows quadratically with languages added.

**Guess the target by language detection.** Pick the layout under which the converted text
looks most like real words. The best experience when correct. Rejected for v1: it is
exactly the heuristic and dictionary work deferred to Phases 2 and 3. Worth revisiting
then, most likely as the ordering of the candidate list rather than as a replacement for
cycling.

**Cycle for selection, separate double-tap for undo.** The literal reading of the brief.
Rejected once the two collapsed into one rule — implementing both would mean maintaining
a special case that the general rule already produces.

## Related

- [ADR-0004](0004-runtime-derived-layout-tables.md) — where the candidate layouts come from
- [ADR-0007](0007-manual-only-v1.md) — why language detection is unavailable in v1
- [ADR-0003](0003-raw-input-over-low-level-hook.md) — `RegisterHotKey` timing, Phase 0 spike
