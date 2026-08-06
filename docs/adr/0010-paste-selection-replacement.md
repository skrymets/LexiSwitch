# ADR-0010: Replace selections by pasting, not by synthesising keystrokes

## Status

Accepted — supersedes [ADR-0006](0006-uia-first-clipboard-fallback.md) in part.

ADR-0006 decided how a selection is **read**, and that decision stands: UI Automation first,
clipboard round-trip as fallback. What changes here is how the replacement is **written**.

## Context

ADR-0006 describes the UI Automation path as one that can "read and replace the selection".
Only the first half was ever true. `TextPattern` is read-only — it can report what is
selected but cannot change it — so replacement has always been keystroke synthesis on both
paths. The write mechanism was never a decision anyone made; it was inherited from last-word
correction, where it is the right tool because the buffer already knows what was typed and
the text is one word long ([ADR-0005](0005-bounded-in-memory-keystroke-buffer.md)).

Applied to a selection, that inheritance produces two defects.

**Undo granularity.** `inject::replace` expands the converted text into two synthetic key
events per UTF-16 unit — about 140 events for a 70-character sentence — and the target
records each character as its own edit. A user who presses `Ctrl+Z` once after a conversion
expects their original text back and gets one character back instead. This is deterministic,
present on every conversion, and immediately visible. It is the stronger of the two defects
precisely because it needs no measurement to observe.

**Delivery is neither atomic nor verifiable.** `SendInput` reports how many events it
inserted into the input stream, not how many the target acted on, so a write that the target
mangles or discards is indistinguishable from one that succeeded. On the automation path the
result is reported as a silent success — which is why the reported symptom was that nothing
happened at all, rather than an error.

An unattended probe reproduced the corruption, dropping characters and repeating one, with no
hotkey and no modifier held. The measurements are recorded in
[selection replacement delivery](../spikes/2026-08-06-selection-replacement-delivery.md);
what this decision rests on is three of their conclusions. The loss is **real and
intermittent**. It is **not shown to be application-specific** — the sample cannot separate
one editor from another, so a fix keyed to a named application would rest on a distinction the
evidence does not support. And it is **not prevented by pacing**, at any batch size or delay
tested. The behaviour is consistent with an application consuming input asynchronously under
load, which is not a property this program can detect in advance or ask about.

A single `SendInput` call is worth distinguishing from an atomic edit here. The whole
replacement already goes out in one call; the call merely enqueues 140 events that the target
then consumes one at a time. Batching at the sending end buys nothing.

## Decision

Replace a selection by **placing the converted text on the clipboard and sending `Ctrl+V`**,
then restoring the previous clipboard contents. This applies to both paths — the automation
read and the clipboard read alike.

Specifics:

- The pasted payload is written with the Clipboard History, Cloud Clipboard, and monitor
  exclusion formats set. Content this program derives from keystrokes is never placed without
  them.
- If the exclusion cannot be applied, **abort rather than paste unprotected**. The clipboard
  is left empty and the failure is reported.
- The previous clipboard text is snapshotted and restored, and non-text formats are left
  rather than reconstructed — ADR-0006's handling, unchanged.
- If the clipboard cannot be acquired after retries, **abort without pasting** — ADR-0006's
  rule, unchanged, and now reaching conversions that previously could not fail this way.

**Last-word correction is out of scope and unchanged.** It needs backspaces to delete what it
replaces, which a paste cannot supply, and it operates on a single word where the undo cost is
bounded and the exposure to per-character loss is small. The same defect exists there in
lesser form and is knowingly left standing.

## Consequences

**Positive.** A conversion becomes one undo step, which is what a user pressing `Ctrl+Z`
means. Delivery becomes atomic, so per-character loss, repetition, and reordering stop being
possible rather than becoming rarer — a class of bug removed instead of mitigated. Long
selections get faster, since cost stops scaling with character count. And an unverifiable
write is replaced by one whose failure modes are ones the program can actually detect and
report.

**Negative, and this is the substantive reversal.** ADR-0006 bought a specific property: on
the automation path the clipboard is *never touched*, so clipboard managers and Clipboard
History never see the text. That property is now gone. Every selection conversion touches the
clipboard.

What replaces it is weaker in kind, not merely in degree. "Never touched" was a structural
guarantee — no data existed for anything to capture. "Touched, with opt-out formats set" is a
cooperative one: Windows Clipboard History and Cloud Clipboard honour those formats, and a
third-party clipboard manager is free to ignore them. The exposure window is short and the
common consumers comply, but the guarantee now depends on other software behaving, and it
did not before. Anyone weighing this ADR should weigh that sentence rather than the format
names.

**Other costs.** Non-text clipboard contents are now destroyed by every selection conversion
rather than only by fallback ones; ADR-0006 already accepted that trade for the fallback, and
this widens its reach. A busy clipboard can now fail a conversion that would previously have
succeeded through automation. And the target must honour `Ctrl+V`, which is near-universal but
not guaranteed — an application that rebinds paste will not be replaced correctly, where
keystroke synthesis would have worked.

**The privacy claim in `respect_password_fields` and the buffer rules is untouched.** Nothing
here changes what is captured or retained; it changes only how already-converted text is
delivered to the application the user is looking at.

## Alternatives Considered

**Pace the keystroke synthesis.** Send the text in smaller batches with delays. Rejected on
measurement — see finding 3 of the
[delivery findings](../spikes/2026-08-06-selection-replacement-delivery.md): every pacing
tested produced corruption, and the one failure in an otherwise clean run happened at an
intermediate pacing while the unpaced batch survived. It also leaves undo granularity exactly
as it is, which is the defect that needs no statistics to see. This was the fix expected
before the measurements were taken, and it would have presented as working.

**Verify after writing, and retry.** Read the text back through `TextPattern`, compare, retry
on mismatch. Attractive because it preserves the clipboard-free property. Rejected: it can
only verify where a text pattern exists, it cannot repair undo granularity, and a retry after
a partially-applied write risks compounding the damage rather than correcting it. Worth
revisiting as a *complement* if the paste path ever needs a correctness check.

**`WM_SETTEXT` / `EM_REPLACESEL`.** Direct and atomic. Rejected on coverage: they reach native
edit controls only, and fail on exactly the Chromium and Electron surface that ADR-0006 exists
to serve.

**Paste without the exclusion formats.** Simpler. Rejected: it puts keystroke-derived text
into Clipboard History and Cloud Clipboard, which is a route to disk and to another machine.

**Do nothing.** Rejected: undo granularity is deterministic, user-visible, and independent of
the corruption question. It would remain a defect even if synthetic input were perfectly
reliable.

## Related

- [Spike findings: selection replacement delivery](../spikes/2026-08-06-selection-replacement-delivery.md) — the measurements this decision rests on, and what they do not establish
- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — why last-word correction needs none of this, and why it keeps keystroke synthesis
- [ADR-0006](0006-uia-first-clipboard-fallback.md) — the read strategy, which stands; and the clipboard-free property this ADR gives up
- [ADR-0008](0008-tray-and-toml-configuration.md) — where the clipboard fallback is disabled
- [Spec: manual-correction](../../openspec/changes/add-manual-layout-conversion/specs/manual-correction/spec.md) — selection and clipboard failure behavior, which needs an undo-granularity scenario added
