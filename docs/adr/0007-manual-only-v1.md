# ADR-0007: Manual hotkeys only in v1; automatic switching deferred

## Status

Accepted

## Context

Punto Switcher's headline feature is silent automatic correction: it watches what you
type, decides a word is layout-mistyped, and fixes it without being asked. That behavior
is the reason people install it.

It is also where the work concentrates. Automatic detection needs per-language impossible
bigram and trigram tables or word dictionaries, a confidence threshold, word-boundary
handling, and — the expensive part — tuning against real typing until false positives are
rare. Dictionaries add a sourcing and licensing question of their own.

False positives are not a minor annoyance. A tool that silently mangles a correct word,
a technical term, a URL, or a password fragment loses the user's trust immediately and
permanently. The failure is silent and destructive, and the user's only recourse is to
notice and undo it.

Manual conversion has no such failure mode. Nothing happens unless a hotkey is pressed;
a wrong result is immediately visible and immediately reversible.

Everything automatic switching needs — accurate layout tables, a keystroke buffer, word
extraction, correct backspace-and-retype injection, exclusion rules — is also needed for
manual conversion. The manual version is not a detour toward the automatic one; it is its
foundation, exercised daily.

## Decision

**v1 ships manual hotkey conversion only.** No automatic detection, no heuristics, no
dictionaries.

Three actions, all explicitly triggered:

- convert the last typed word, in place;
- convert the current selection, in place;
- convert the clipboard contents in place, without pasting.

Automatic switching is deferred to phases documented in the spec:

- **Phase 2** — heuristic auto-switch: impossible bigram and trigram tables per language,
  a confidence threshold, correction on word boundary. No dictionaries.
- **Phase 3** — dictionary auto-switch: bundled uk/en/de word lists, a learned-word and
  exception store, per-application enablement.
- **Phase 4** — settings GUI (see [ADR-0008](0008-tray-and-toml-configuration.md)).

## Consequences

**Positive.** v1 is roughly 40% of the total scope and reaches daily usability far sooner.
Zero false positives by construction — the tool acts only when told. Every foundation
component gets exercised in real use before anything is built on top of it, so Phase 2
starts from a validated base rather than an assumed one. The riskiest and least
predictable work is deferred until the boring work is known-correct.

**Negative.** v1 is less convenient than Punto for its central use case: the user must
notice the mistake and press a key. That is the deliberate trade — noticing and pressing
a key is a small cost, and it is the cost that buys the absence of silent corruption.

**Deferred, not dropped.** The phases are recorded here and in the spec so the
architecture stays hospitable to them. In particular, [ADR-0003](0003-raw-input-over-low-level-hook.md)
was checked against this: retroactive correction by backspace-and-retype means automatic
switching never requires keystroke suppression, so Raw Input remains sufficient through
Phase 3.

## Alternatives Considered

**Manual plus heuristic auto-switch in v1.** Useful from day one without dictionary
sourcing. Rejected: heuristics need tuning against real typing, and the tuning data does
not exist until the tool is in daily use — which requires shipping something first. Also
front-loads the false-positive risk before the foundation it depends on has been proven.

**Full Punto-equivalent v1 with dictionaries.** Most capable. Rejected: the largest
possible v1, and it drags in word-list licensing and a persistence layer before the core
conversion path has been validated in real use even once.

## Related

- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — the buffer is the only passive component while v1 is manual
- [ADR-0009](0009-multi-tap-layout-cycling.md) — how the manual actions choose a target layout
- [Change: add-manual-layout-conversion](../../openspec/changes/add-manual-layout-conversion/proposal.md) — what v1 delivers
