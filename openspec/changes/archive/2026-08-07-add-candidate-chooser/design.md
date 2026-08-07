## Context

See proposal.md for motivation. Four constraints shape the approach, three of them measured
rather than assumed.

**The chooser must be a focused window.** A keyboard-driven chooser has to receive keys, and
the only alternative that avoids taking focus — a non-activating window fed by a low-level
keyboard hook — is forbidden by [ADR-0011](../../../docs/adr/0011-no-keyboard-observation.md).
Registering the navigation keys as global hotkeys is not a serious substitute.

**The selection survives the round trip.** Measured before this design was written, across a
Win32 control with a text pattern, a Scintilla control without one, and a Chromium text field.
Handing the foreground back needed no settle at all
(`docs/spikes/2026-08-07-focus-round-trip.md`).

**The read path is not stable per application.** The same Chromium window exposed no text
pattern on one run and a working one on the next.

**The replacement path already exists and is sufficient.**
[ADR-0010](../../../docs/adr/0010-paste-selection-replacement.md) delivers replacement as one
logical edit, verified across both read branches.

## Goals / Non-Goals

**Goals**

- Candidate generation, filtering, deduplication and ranking as pure logic in
  `lexiswitch-core`, testable against the frozen fixtures without Windows.
- A transaction whose every outcome is distinguishable by the user.
- No text modified before commit, under any code path.

**Non-Goals**

- Near-caret placement. A stable palette on the active monitor first; caret geometry,
  screen-edge repositioning and overlay-covering-the-selection are deferred.
- Learning from previous choices. Deferred, and if added must only affect ordering.
- Suspicious-segment detection. The selection is the user's declaration of scope.
- Shortcut conflict UX and tray state rework — a separate change.

## Decisions

**Candidate generation lives in `lexiswitch-core` and is nearly free.** `convert` already
returns `None` when a conversion changes nothing, and characters absent from the source layout
already pass through. Generation is therefore: every ordered pair of distinct layouts, mapped
through the existing converter, keeping the results that came back `Some`. The filtering rule
the pivot asks for falls out of behavior that is already tested rather than being added on top.

**Deduplicate on the pair of result and target layout, not on the result.** With en-US and
de-DE installed, `uk→ENG` and `uk→DEU` produce identical text for any selection avoiding the
transposed keys and differing punctuation. The target is exactly what a layout-changing commit
acts on, so collapsing on visible text alone would silently discard a choice the user is being
asked to make. The cost is rows that look identical; the source and target labels are what
distinguish them, and that is honest about what differs. Merging into one row with a target
sub-choice was considered and rejected as a second selection dimension inside a chooser whose
value is that it has one.

**Ranking leans on whether the source alphabet disambiguates, and admits when it does not.**
Cyrillic identifies Ukrainian immediately; Latin never separates en-US from de-DE, and the
constraint against dictionaries means it never will. So en↔de preselection cannot be better
than a coin flip, and the design does not pretend otherwise — hence *detected* versus
*suggested* rather than a confidence score the system cannot justify.

**Read the selection before the chooser opens, while the user's application still has focus.**
The alternative — opening first and reading after — would require reading a selection in a
window that no longer has focus, which is a strictly harder problem for no benefit.

**Choose the read path per invocation and never cache it.** Chromium's text pattern availability
was observed changing between consecutive runs. A cached per-application decision would be
wrong roughly half the time, and detect-by-attempt costs one failed call.

**Build the chooser on raw Win32.** The single-executable requirement and the 5 MB budget
(currently 0.57 MB) rule out WinUI, WPF and the Rust immediate-mode toolkits, which pull in a
graphics stack. That leaves the approach already used for the tray and the message window. It
is the largest piece of hand-written UI in the project and should be treated as such in
estimates.

## Risks / Trade-offs

**The chooser is the dominant cost of the pivot** → It is one change, separable from the
removals, and the removals ship first so the product stays usable while it is built.

**A palette on the active monitor is far from the text being edited** → Accepted deliberately.
Near-caret placement adds caret detection, selection geometry, edge repositioning and overlays
covering the very text under review. Revisit once the stable palette is proven.

**Latin-to-Latin preselection is unreliable** → Named as a limitation in the spec rather than
hidden behind a confidence figure. The user's eye is the tiebreaker, which is why all
candidates are visible at once.

**A commit now costs two keystrokes where cycling cost one** → The trade the pivot chose:
nothing is modified before an explicit commit, so a one-keystroke path cannot exist without
reintroducing silent modification.

**Selection survival was measured on three targets, not all of them** → VS Code is not
installed on the development machine and Windows Terminal needs a differently shaped probe.
Both gaps are recorded in the spike findings and belong to the compatibility matrix, not to
model viability.

## Open Questions

**Where a chooser row's compact preview should start when the changed region is far into a long
selection.** The spec requires that omissions be visible and that the window favour changed
text; the exact algorithm can be chosen during implementation without changing the specs, the
approach, or the task breakdown.
