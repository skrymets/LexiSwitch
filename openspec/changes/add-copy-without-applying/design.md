## Context

See `proposal.md` — Why. What shapes the approach is that the clipboard is not free real
estate: the transaction is already using it, and already has a plan for putting it back.

`Transaction` (`lexiswitch-win/src/transaction.rs`) holds `clipboard_before` — what the
clipboard held before the read disturbed it — and `clipboard_disturbed`, whether the read
touched it at all. `cancel()` calls `put_clipboard_back()`; `commit()` hands the same value to
`selection::paste_over_selection`, which restores it after the paste lands. Both would overwrite
a copy the user made in between, one immediately and one a moment later.

The windows, by contrast, are deliberately powerless: each is given the rendered results and
nothing that could modify a document. That is worth preserving — this adds the clipboard to what
a window may touch, and nothing else.

## Goals / Non-Goals

**Goals:**

- Copying costs one keystroke from where the user already is, and leaves the document alone.
- A copy survives whatever the transaction does next, including a commit.
- The exposure choice is visible, defaulted to the safe answer, and cannot be inherited.
- The two windows behave identically, and share the code rather than resembling each other.

**Non-Goals:**

- No copying of the *original* selection — the user already has it selected and `Ctrl+C` in
  their own application does that.
- No copy history, no multiple slots, no accumulating several results.
- No new configuration key. The exposure choice is per-transaction by decision, not by omission.
- No change to how the selection is read or how a replacement is written.

## Decisions

### The copy happens immediately, not when the window closes

Deferring it until the window returns would be simpler — no clipboard code in the window at all
— and, because the window is modal, indistinguishable to the user.

Rejected because the window must confirm the copy, and this project does not report success it
has not observed. A deferred copy can only be reported optimistically; an immediate one can be
reported truthfully, and a failure can be shown while the user is still looking at the window
that could retry it. The window gains the owner handle and the retry count `Clipboard::open`
needs, and still cannot reach the document.

### `clipboard_before` becomes *what to restore*

The field currently means "what was there beforehand", and after a deliberate copy that is no
longer what should be put back. Rather than adding a second field that competes with it, the
field's meaning changes and its name changes with it.

Leaving a field called `before` holding text captured in the middle of the transaction is
precisely how the next reader gets this wrong, and the rename is the cheap part of the work.

It also makes the commit path fall out correctly without further thought: `commit()` already
prefers the stored value when the clipboard was disturbed, so a copy that sets both the value
and the disturbed flag is restored after the paste with no change to the paste path's logic.

### The restore carries the exclusion flag

`put_clipboard_back` restores with plain `set_text`, and so does the paste path. A copy the user
asked to keep out of Clipboard History would therefore be put *into* history at the end of the
transaction — silently undoing the toggle, at the one moment nobody is looking.

So what the transaction restores is a pair: the text and whether it may be exposed. Text that
was there before the transaction keeps the existing behaviour, because it was already in the
user's history before this program touched it.

### The exposure toggle is `H`, and it is not a seventh strip option

The case window numbers its strip options `1`–`6`, and a seventh digit would be the obvious
place to put this. It is the wrong place: those six change *the text*, and this changes only
what happens to a copy of it. Sharing their row and their numbering would imply it alters the
result.

`H` instead, in both windows, shown beside the copy hint in the footer. One key in both is what
makes it learnable, and the chooser has no digit strip to extend in any case.

### Changing the toggle after a copy re-copies

Otherwise the clipboard holds text under the old setting while the footer states the new one,
and the two disagree with no way for the user to tell. The text is already in hand, so bringing
the clipboard into line costs one line of code and removes the discrepancy rather than
documenting it.

### Copying does not count as using a style

The promotion counts exist to order the list by what the user applies to documents. A copy is
not an application, and counting it would let a style the user never applied climb to the top.

## Risks / Trade-offs

**The restore path is on the commit path.** → This reaches the code that writes to the user's
document. The change is to *what* is put back rather than to the paste mechanism, but the full
application matrix is the gate, not a smoke run.

**A copy defeats the clipboard-minimisation intent.** Converted text now stays on the clipboard
after the transaction, where before it was cleaned up. → That is the feature. The exclusion
default keeps it out of history, and it is only there because the user asked.

**Two windows, one behaviour, and a history of them drifting.** → Whatever is genuinely common
goes into `winui` rather than being written twice. The recorder reproducing the chooser's
activation bug is the precedent, and it was a copy of exactly this size.

**`Ctrl+C` in a window that is showing the user's text is a plausible accident.** → Harmless: it
writes the clipboard and nothing else, the document is untouched, and the confirmation says what
happened. There is no state to undo.

## Open Questions

- **Whether Clipboard History actually skipped the entry is not observable from a script.** The
  shell check can assert the exclusion format was registered and the write used it; that the
  entry is absent from the history UI needs a person, and belongs in the manual checklist rather
  than in an assertion that would be claiming more than it measured.
