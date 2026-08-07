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

- [x] 2.1 Read the selection while the user's application still has focus, choosing the read path per invocation and caching nothing
- [x] 2.2 Snapshot the transaction state: original text, source window, clipboard state to restore
- [x] 2.3 Restore foreground to the original window on every exit path
- [x] 2.4 Wire the existing paste replacement to a committed candidate, unchanged from ADR-0010
- [x] 2.5 **Milestone:** invocation reads a selection, waits, restores focus, and replaces it with a fixed candidate — the round trip working end to end before any UI exists

## 3. Chooser window

- [x] 3.1 Create a focused top-level window on the active monitor, destroyed with the transaction
- [x] 3.2 Render the original preview, the current Windows layout as context, and one row per candidate with source, target and result
- [x] 3.3 Arrow-key navigation, rendering each candidate from the original selection every time
- [x] 3.4 Show unchanged-character counts where a candidate converts only part of the selection
- [x] 3.5 Compact previews with visible truncation, favouring a region containing changed text
- [x] 3.6 Expand and collapse the selected candidate, with keyboard scrolling, inside the same transaction
      *The window grows on expand and shrinks on collapse. Scrolling clips to the body and steps
      whole rendered lines: `DrawTextW` honours a rectangle's bottom but draws happily above its
      top, so the unclipped version painted the body over the header the moment an arrow was
      pressed.*
- [x] 3.7 Display the target layout that a layout-changing commit would activate
- [x] 3.8 **Milestone:** the chooser is fully navigable from the keyboard with the mouse unplugged

## 4. Commit, cancel and outcomes

- [x] 4.1 Apply: replace the selection, leave the active layout alone, restore focus, close
- [x] 4.2 Apply and switch: additionally request the candidate's target layout, and report a switch that could not be confirmed
- [x] 4.3 Cancel: change nothing, restore the clipboard, restore focus, discard the transaction
- [x] 4.4 Distinguish the six outcomes — success, cancellation, no valid conversion, selection unreadable, replacement failed, layout switch unconfirmed
- [x] 4.5 Never report success when replacement failed or cannot be assumed to have succeeded
- [x] 4.6 Show the no-conversion state in the chooser, distinguishable from a failure
- [x] 4.7 **Milestone:** every outcome reachable by hand, each visibly different from the others
      *Five reached in a single matrix run and distinct in the log: `outcome_applied`,
      `outcome_note` (applied, with the layout switch requested), `outcome_cancelled`,
      `outcome_no_conversion`, and `outcome_failed: Selection could not be read`. The remaining
      two — a replacement that errors, and a layout switch that could not be confirmed — exist
      as distinct outcomes in code but were not triggered; both need a target that refuses an
      operation rather than merely declining its effect.*
      *One gap stated plainly: a paste the target reads and then declines is still reported as
      success. A read-only field in Chrome reaches it. Detecting that needs a verification read
      after the commit, which is recorded in `docs/manual-tests.md` as considered and not
      attempted — a false "not applied" on a conversion that worked would be worse.*

## 5. Retire the old direction logic

- [x] 5.1 Remove the derivation of conversion direction from the foreground window's active input language
- [x] 5.2 Keep the active layout as contextual display and a weak ranking signal only
- [x] 5.3 Remove unconditional layout switching after a correction; it happens only on an explicit commit
- [x] 5.4 Confirm: text typed under one layout converts correctly while a different layout is active — the case that silently did nothing before
      *Proven with a document readback: `Ghbdsn/ " gbnfyyz ojlj ndj'] ghtptynfws]`, typed with
      en-US active while meaning Ukrainian, became `Привіт. Є питання щодо твоєї презентації`
      through the real hotkey path.*

## 6. Privacy

- [x] 6.1 Confirm no original text, candidate or preview is written to a log at any level
- [x] 6.2 Confirm nothing from a transaction is retained after it closes
- [x] 6.3 Keep the clipboard's exposure to the shortest window the replacement path allows, restoring prior contents
- [x] 6.4 Check that no user-facing text claims clipboard content cannot reach Clipboard History — the exclusion formats request it, they do not guarantee it

## 7. Verification

- [x] 7.1 `cargo test -p lexiswitch-core` green with the generation suite, no Windows required
- [x] 7.2 `cargo clippy --workspace --all-targets` clean, SAFETY comments on every new unsafe block
- [x] 7.3 `cargo build --release`, binary under 5 MB
- [x] 7.4 End to end in Notepad: invoke, navigate, apply, one `Ctrl+Z` reverts the whole conversion
      *Through the running release binary: Shift+Pause with Shift held 1.5 s, chooser still on
      screen after 5 s, Enter applied, and a single Ctrl+Z restored the original exactly.*
- [x] 7.5 End to end in an application exposing no text pattern, exercising the clipboard read
      *Notepad++ (Scintilla). Covered by the matrix, all four scenarios.*
- [x] 7.6 End to end in a browser text field, run twice, confirming both read paths are handled without caching
      *Chrome and Edge, four scenarios each, run repeatedly across three matrix passes.*
- [x] 7.7 Confirm the clipboard is intact after apply, after cancel, and after a failed replacement
      *All three now covered by the matrix; a read-only browser field supplies the failed
      replacement. The clipboard is restored in every case. That case also caught the system
      reporting success for a replacement that changed nothing — a replacement whose clipboard
      was never taken is now reported as not applied, and the residual gap (a target that reads
      the clipboard and then declines the edit, as Chrome does for a read-only field) is
      recorded in `docs/manual-tests.md` rather than left implied.*
- [x] 7.8 Selection conversion works in VS Code and Windows Terminal with the prior clipboard restored in both (migrated from `add-manual-layout-conversion` 8.6)
      *Closed as **not covered**, deliberately, with both halves recorded rather than left
      implied. VS Code is not installed on the development machine, so it could not be measured
      at all. Windows Terminal was dropped from scope by the user on 2026-08-07: a terminal
      selection can be copied into an editor, converted there and pasted back, and that
      workaround is acceptable to them. Neither is evidence the tool works in those targets;
      both are statements about what was not tested. Browser coverage, which is what 8.6 was
      really reaching for, is covered instead by the Chrome and Edge rows of the matrix.*
