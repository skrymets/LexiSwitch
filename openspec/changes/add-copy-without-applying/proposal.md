## Why

Both transaction windows can do exactly one thing with a result: put it into the document.
There is no way to take a conversion or a re-casing *out* — to convert some text and paste the
result somewhere else while leaving the original where it is.

That is a real gap rather than a nicety. Correcting a mistyped phrase to quote it in a message,
turning a heading into `snake_case` for a filename, converting something to check what it looks
like — all of them currently require applying the change to the document, copying it, and then
undoing. Three destructive steps to get a string the program had already computed and was
showing on screen.

The window is the natural place for it: the result is already rendered, already selected, and
already a keystroke away.

## What Changes

- **`Ctrl+C` and `Ctrl+Insert` copy the selected result** in both the layout chooser and the
  case window. The document is not touched and the window stays open — closing is still `Esc`,
  and applying is still `Enter`.
- What is copied is the **full result**, not the truncated preview, and the expanded view copies
  the same thing the list does.
- **A history toggle, `H`, in both windows.** Off by default, so a copy is excluded from Windows
  Clipboard History the way the paste path already excludes converted text; on, and it behaves
  like an ordinary copy the user made themselves. It resets every time the window opens, like
  the case window's strip options, so a destructive-by-omission setting cannot be left armed.
- **The transaction stops restoring over a deliberate copy.** Today cancelling puts back the
  clipboard the transaction disturbed, and committing restores it after pasting. Either would
  wipe what the user had just copied — cancelling immediately, committing a moment later.
- **`allow_clipboard_fallback = false` does not block it.** That setting governs what the
  program does behind the user's back when reading a selection; `Ctrl+C` is an instruction.

Not breaking. Nothing that works today behaves differently unless the copy key is pressed.

## Capabilities

### New Capabilities

None. This adds an action to two transactions that already exist rather than a new capability.

### Modified Capabilities

- `candidate-chooser`: gains copying as an action, and the two requirements that currently
  forbid it have to say so. **Cancelling changes nothing** promises the clipboard is restored to
  what it held before; **Nothing from a transaction is retained** promises the previous clipboard
  state is restored where possible. A copy the user asked for is an exception to both, and it has
  to be written as an exception rather than left as a contradiction.
- `case-conversion`: the same action, and the same two amendments — the cancel clause lives in
  **Applying is explicit and changes no keyboard layout**, and the retention clause in **Nothing
  from a case transaction is retained**.

## Impact

**Modified.** `lexiswitch-win`: `transaction.rs` (what the transaction restores, and how),
`selection.rs` (the paste path's restore carries the exclusion flag), `chooser.rs` and
`casewindow.rs` (the keys, the toggle, the confirmation), `winui.rs` (whatever the two windows
would otherwise copy from each other). `lexiswitch`: `worker.rs` (hand the copy to the
transaction before ending it).

**Unmodified.** `lexiswitch-core` — there is no new logic here, only a new thing to do with a
string that is already computed. `clipboard.rs` already has both `set_text` and
`set_text_excluded`; the excluded variant exists for the paste path and is exactly what the
default needs.

**Risk.** The restore path is on the commit path, so this reaches the code that writes to the
user's document. The change is to *what* is put back rather than to the paste itself, but the
full application matrix is the gate rather than a smoke run.

**Budgets.** No new dependency, no new window, no measurable size or memory cost.
