# ADR-0014: Case conversion is a separate transaction

## Status

Accepted. Supersedes nothing. [ADR-0012](0012-single-chooser-transaction.md)'s rule — one
invocation opens one chooser showing every distinct candidate at once — stands untouched and
is extended to a second kind of invocation rather than stretched over two kinds at once.

## Context

LexiSwitch converts text typed under the wrong keyboard layout. Changing the **case** of
selected text is a different operation that wants the same machinery: read the selection,
show what each transform would produce, replace on an explicit commit. Every expensive part
is already built and already verified across the application matrix.

The economical move is therefore to add case transforms as further candidates in the
existing chooser, behind the existing shortcut. That was the starting assumption, and it
does not survive contact with how candidates are generated.

**Both families are always non-empty, so one of them is always noise.**
`candidates::generate` tries every ordered pair of installed layouts and keeps the ones that
change the text. Ukrainian prose can be reinterpreted as en-US and as de-DE, so it always
yields two layout candidates — including when the text is perfectly correct and the user
wanted `UPPERCASE`. Case transforms apply to any text at all, including mis-typed garbage
like `Ghbdsn/ " gbnfyyz`, where every one of them is worthless. With thirteen case styles
and two or three layout candidates, roughly eight of fifteen rows are irrelevant on every
single invocation, and *which* eight depends on an intent the text does not carry.

**Ranking cannot resolve it, because the signal does not exist.** `candidates::rank` orders
by evidence available in the characters themselves. Nothing in the characters says whether
the user pressed the shortcut to fix a layout or to change case. A ranking that guessed
would be wrong about half the time and would move the correct row up and down between
invocations, which is worse than a stable wrong order.

**Separating them inside one window means paging, which is already forbidden.** The
`candidate-chooser` specification requires that the system *"SHALL NOT present candidates as
pages."* That requirement was written against multi-tap cycling, but a `Layout | Case` tab
bar is the thing it describes, and honouring the letter while breaking the intent is not
available — it would have to be superseded.

Four in-window arrangements were mocked and compared before this record was written: a flat
grouped list, a flat grouped list with type-to-find narrowing, a collapsed drill-in row, and
tabbed pages. Each trades the noise for either extra keystrokes on the frequent path or a
new interaction concept in a window that currently has none.

## Decision

**Case conversion is its own transaction: its own shortcut, its own window, its own
candidate set.** The two never appear in one list.

Concretely:

- A second global shortcut, defaulting to `Win+Ctrl+Pause`, opens a case window. The
  existing shortcut and the existing chooser behave exactly as before.
- `chooser.rs`, `candidates.rs` and `convert.rs` are not modified, and the
  `candidate-chooser` specification is not amended.
- Both transactions share what is genuinely common and nothing else: `Transaction` for
  reading, committing and cancelling; `winui` for window chrome, fonts, placement and the
  foreground handshake; `preview` for fitting a result into a row.
- Case candidates are a **fixed list that is never filtered**. Unlike layout candidates,
  which are discovered and unnamed, every case style has a name the user aims at. Rows that
  would change nothing are dimmed, not removed, so the list never changes shape and a style
  stays where the hand learned to find it.

## Consequences

**Positive.** Every row in either window is relevant to the invocation that opened it. The
layout path — the frequent one, and the one this product exists for — is unchanged in
keystrokes, in appearance and in risk; no regression to it is possible from this work. The
chooser's specification and its hard-won behaviour around focus, activation and commit stay
frozen.

Because the case set is fixed and named, "often used" ordering and per-style memory become
meaningful, which they could not be for a discovered candidate set.

**Negative.** A second global shortcut is a second thing that can be held by another
application. The existing conflict state, tray fallback and shortcut recorder all apply, but
`Hotkeys::register` is currently all-or-nothing and must become per-binding — otherwise a
conflict on the case shortcut would silently disable layout conversion, which is the exact
failure this decision is supposed to prevent.

Two windows now share near-identical chrome. That duplication is real, and the mitigation is
to lift the shared parts into `winui` rather than to copy them: the shortcut recorder
already reproduced the chooser's foreground-activation bug once, precisely because the rule
lived in the chooser's own state instead of the shared module. This is the third window.

The user has to remember which shortcut is which. Two keys on the same physical `Pause` key
is the mitigation, not a fix.

**Unresolved.** Chaining — fixing the layout and then changing the case in one gesture — is
out of scope. After a commit the text is no longer selected and the caret sits at the end
([ADR-0012](0012-single-chooser-transaction.md) records what that costs), so a second
transaction cannot simply reuse the first one's selection. Deciding this needs the selection's
post-commit state to be established, which it is not.

## Alternatives Considered

**One flat list, grouped by family.** Everything visible, `Tab` jumping between group heads.
No new interaction concepts and no conflict with the no-paging requirement. Rejected: pays
the full noise cost on every invocation, including the frequent layout path, and reaching
`KEBAB-UPPERCASE` takes twelve arrow presses.

**One flat list with type-to-find narrowing.** The above plus incremental filtering, so any
style is directly addressable in two or three keystrokes. Genuinely the fastest for a known
target. Rejected with the flat list it depends on; it also costs `Space`, which currently
means expand, since a filter needs the space character.

**A collapsed "Change case ▸" row that drills in.** Keeps the layout view exactly as it is
and puts case one key away. The strongest in-window option. Rejected because it is a page by
another name — the thing the specification forbids — and because it still shares one shortcut
between two intents, so the window must open in one of them and be wrong half the time.

**Tabbed pages, `Layout | Case`.** Rejected outright: it is what the no-paging requirement
describes, and superseding a requirement to gain an arrangement that is not better than the
alternatives above is a poor trade.

**Deciding the family from the text.** Rejected: the intent is not in the characters. See
Context.

## Related

- [ADR-0012](0012-single-chooser-transaction.md) — the one-invocation-one-chooser rule this extends to a second invocation
- [ADR-0011](0011-no-keyboard-observation.md) — why any invocation is explicit and operates on a selection
- [ADR-0010](0010-paste-selection-replacement.md) — the commit path both transactions share
- [ADR-0015](0015-usage-memory-outside-configuration.md) — where the "often used" ordering is kept
