## 1. Candidate generation — `lexiswitch-core`, test-first

Pure logic over layout tables. Every test here must pass without Windows, against the frozen
fixtures.

- [x] 1.1 Failing test: every ordered pair of distinct installed layouts is considered, and no self-pair is
- [x] 1.2 Implement pair enumeration over the layout table set
- [x] 1.3 Failing test: a candidate whose result equals the original is discarded, including the case where the source layout produces none of the selected characters
- [x] 1.4 Implement filtering on top of the existing converter, which already reports "no change"
- [x] 1.5 Failing test: two candidates with identical text but different targets are both retained; two with identical text and the same target collapse to one
- [x] 1.6 Implement deduplication keyed on result and target together
- [x] 1.7 Failing test: a Cyrillic selection ranks Ukrainian-sourced candidates above candidates whose source cannot produce those characters
- [x] 1.8 Failing test: a Latin selection with en-US and de-DE installed produces a stable, deterministic order without claiming a source
- [x] 1.9 Implement ranking from the deterministic signals only
- [x] 1.10 Failing test: unchanged-character counts are reported per candidate
- [x] 1.11 **Milestone:** generating candidates for the fixture selections produces exactly the expected set, ordering included, with no Windows involved

## 2. Transaction plumbing — `lexiswitch-win`

- [ ] 2.1 Read the selection while the user's application still has focus, choosing the read path per invocation and caching nothing
- [ ] 2.2 Snapshot the transaction state: original text, source window, clipboard state to restore
- [ ] 2.3 Restore foreground to the original window on every exit path
- [ ] 2.4 Wire the existing paste replacement to a committed candidate, unchanged from ADR-0010
- [ ] 2.5 **Milestone:** invocation reads a selection, waits, restores focus, and replaces it with a fixed candidate — the round trip working end to end before any UI exists

## 3. Chooser window

- [ ] 3.1 Create a focused top-level window on the active monitor, destroyed with the transaction
- [ ] 3.2 Render the original preview, the current Windows layout as context, and one row per candidate with source, target and result
- [ ] 3.3 Arrow-key navigation, rendering each candidate from the original selection every time
- [ ] 3.4 Show unchanged-character counts where a candidate converts only part of the selection
- [ ] 3.5 Compact previews with visible truncation, favouring a region containing changed text
- [ ] 3.6 Expand and collapse the selected candidate, with keyboard scrolling, inside the same transaction
- [ ] 3.7 Display the target layout that a layout-changing commit would activate
- [ ] 3.8 **Milestone:** the chooser is fully navigable from the keyboard with the mouse unplugged

## 4. Commit, cancel and outcomes

- [ ] 4.1 Apply: replace the selection, leave the active layout alone, restore focus, close
- [ ] 4.2 Apply and switch: additionally request the candidate's target layout, and report a switch that could not be confirmed
- [ ] 4.3 Cancel: change nothing, restore the clipboard, restore focus, discard the transaction
- [ ] 4.4 Distinguish the six outcomes — success, cancellation, no valid conversion, selection unreadable, replacement failed, layout switch unconfirmed
- [ ] 4.5 Never report success when replacement failed or cannot be assumed to have succeeded
- [ ] 4.6 Show the no-conversion state in the chooser, distinguishable from a failure
- [ ] 4.7 **Milestone:** every outcome reachable by hand, each visibly different from the others

## 5. Retire the old direction logic

- [ ] 5.1 Remove the derivation of conversion direction from the foreground window's active input language
- [ ] 5.2 Keep the active layout as contextual display and a weak ranking signal only
- [ ] 5.3 Remove unconditional layout switching after a correction; it happens only on an explicit commit
- [ ] 5.4 Confirm: text typed under one layout converts correctly while a different layout is active — the case that silently did nothing before

## 6. Privacy

- [ ] 6.1 Confirm no original text, candidate or preview is written to a log at any level
- [ ] 6.2 Confirm nothing from a transaction is retained after it closes
- [ ] 6.3 Keep the clipboard's exposure to the shortest window the replacement path allows, restoring prior contents
- [ ] 6.4 Check that no user-facing text claims clipboard content cannot reach Clipboard History — the exclusion formats request it, they do not guarantee it

## 7. Verification

- [ ] 7.1 `cargo test -p lexiswitch-core` green with the generation suite, no Windows required
- [ ] 7.2 `cargo clippy --workspace --all-targets` clean, SAFETY comments on every new unsafe block
- [ ] 7.3 `cargo build --release`, binary under 5 MB
- [ ] 7.4 End to end in Notepad: invoke, navigate, apply, one `Ctrl+Z` reverts the whole conversion
- [ ] 7.5 End to end in an application exposing no text pattern, exercising the clipboard read
- [ ] 7.6 End to end in a browser text field, run twice, confirming both read paths are handled without caching
- [ ] 7.7 Confirm the clipboard is intact after apply, after cancel, and after a failed replacement
- [ ] 7.8 Selection conversion works in VS Code and Windows Terminal with the prior clipboard restored in both (migrated from `add-manual-layout-conversion` 8.6; VS Code is not installed on the development machine and Windows Terminal needs a probe that types at a prompt rather than selecting a document — record what could not be covered)
