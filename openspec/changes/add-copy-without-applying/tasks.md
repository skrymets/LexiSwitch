## 1. What the transaction puts back

Nothing here adds a feature. It changes what "restore the clipboard" means, so that a copy has
somewhere to survive, and the product must be fully working at the end of the group.

- [x] 1.1 Rename `Transaction::clipboard_before` to say what it now means — the state to restore, not the state that was there — and pair it with whether that state may enter Clipboard History. Text captured before the transaction is exposable, because it was already in the user's history.
- [x] 1.2 Make `put_clipboard_back` honour the flag, choosing `set_text` or `set_text_excluded`.
- [x] 1.3 Give `selection::paste_over_selection` the flag alongside the text it restores. This is the write at `selection.rs:282` that currently drops it, and the one that would expose a copy at the end of a commit.
- [x] 1.4 Add `Transaction::keep_on_clipboard(text, exposable)`: the user has asked for this to be on the clipboard when the transaction ends. It sets the restore state and marks the clipboard disturbed, so the commit path restores it after pasting without further special-casing.
- [x] 1.5 `cargo test --workspace` and `cargo clippy --workspace --all-targets` green, and a full `run-matrix.ps1` run — the commit path changed and nothing else in this change will exercise it as hard.

## 2. Copying, shared between the two windows

- [x] 2.1 Put the copy itself in `winui`: given the text and whether it may be exposed, write it to the clipboard and report whether that worked. Both windows call it; neither owns it.
- [x] 2.2 `casewindow` and `chooser` each accept the owner handle and retry count `Clipboard::open` needs. Nothing else about what they are given changes — they still cannot reach the document.
- [x] 2.3 `Ctrl+C` and `Ctrl+Insert` copy the selected result in both windows. The window stays open, the transaction stays live, and the selection is untouched.
- [x] 2.4 Copy the whole result, never the shortened preview, and the same text a commit would write. The expanded view copies what the list copies.
- [x] 2.5 `H` toggles whether the copy may enter Clipboard History, in both windows. Off when the window opens, every time.
- [x] 2.6 Changing `H` after a copy re-copies under the new setting, so the clipboard and the footer cannot disagree.
- [x] 2.7 Confirm a copy on screen, and only when the clipboard was actually written. A failed copy says so and leaves the transaction otherwise untouched.
- [x] 2.8 Name both bindings in each window's footer, alongside the state of `H`.
- [x] 2.9 Report what was copied out of `show`, so the caller can hand it to the transaction.

## 3. Wiring

- [x] 3.1 `worker.rs` passes the owner handle to both windows and, on return, calls `keep_on_clipboard` before cancelling or committing — so a copy survives either ending.
- [x] 3.2 A copy is not recorded as a use of the style. The counts order the list by what was applied to a document, and a copy is not that.
- [x] 3.3 Copying works with `selection.allow_clipboard_fallback = false`. That setting governs reading a selection unbidden, not an instruction to copy.

## 4. Verification

- [x] 4.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`, release build under 5 MB.
- [x] 4.2 New Copy group in `docs/testing/shell-checks.ps1`, in both windows: copy then `Esc` leaves the document unchanged and the clipboard holding the result; copy one option then apply a different one, and confirm the document has the applied one while the clipboard keeps the copied one; copying with the reading fallback disabled still works.
- [x] 4.3 Assert the exclusion is *requested*: with `H` off the write uses the excluded form, with `H` on it does not. Whether Clipboard History actually skipped the entry is not readable from a script and must not be asserted as though it were.
- [x] 4.4 Full `run-matrix.ps1`. Mandatory: `transaction.rs` and `selection.rs` are both on the commit path.
- [ ] 4.5 Manual, as group E of `docs/testing/manual-checklist.md`: the footer names `Ctrl+C` and `H`; a copy is confirmed on screen; `H` on, copy, and the entry **does** appear in the Clipboard History panel; `H` off and it does not.
- [x] 4.6 Record results in `docs/manual-tests.md`.
