## 1. Groundwork that leaves the product working

Nothing here adds behaviour. The product must be fully working after this group, so each item
is verifiable on its own before any new surface exists.

- [x] 1.1 Move the armed-foreground handshake out of `chooser.rs` into `winui`: taking the foreground with retries, confirming against `GetForegroundWindow`, and the rule that losing activation only cancels once the window has settled in front. Move `draw`, `draw_right` and `rule` with it. Leave list rendering where it is.
- [x] 1.2 Point `chooser.rs` and `recorder.rs` at the shared handshake and delete their copies. Verify by opening and cancelling both windows: no flash-and-vanish, and clicking away still cancels.
- [x] 1.3 Failing test in `lexiswitch-core`: `changed_span` over a pair where the second string has characters inserted (`hello world` → `hello_world`, and a case where the string grows, `straße` → `STRASSE`) reports a span covering the change. It currently does not.
- [x] 1.4 Reimplement `changed_span` as a common-prefix / common-suffix span. Existing `preview` tests must pass unchanged — the positional case is a special case of the new one.
- [x] 1.5 Failing test: registering two bindings where the second is unavailable leaves the first registered. `Hotkeys::register` currently releases everything.
- [x] 1.6 Make registration per-binding, reporting the outcome of each. Keep `Drop` releasing everything it holds.
- [x] 1.7 Remove the dead `HotkeyId` variants `ConvertLastWord`, `ConvertClipboard` and `PauseToggle`, left behind by the pivot, and their `config_key` arms.
- [x] 1.8 Failing tests in `lexiswitch-core` for `Shortcut`/`AgentState` carrying two bindings: both registered, one conflicted, both conflicted. A conflict names the unavailable combination and states that the other works.
- [x] 1.9 Implement the two-shortcut state, including `summary()` and `explanation()`. Precedence in `resolve` is unchanged; only the shortcut term becomes plural.
- [x] 1.10 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets`. Green before continuing.

## 2. Case transforms in the core, test-first

- [x] 2.1 Failing tests for segmentation: boundaries at whitespace, `_`, `-`, `.`, and lowercase→uppercase; **no** boundary between a letter and an adjacent digit; separators record whether their run contained a line break; leading and trailing separators are preserved as separators.
- [x] 2.2 Implement segmentation into words and separators.
- [x] 2.3 Failing tests for all fourteen styles against a shared fixture set: ASCII prose, Ukrainian prose, German text containing `ß` (`ß`→`SS` grows the string), a single word, an existing identifier, and a multi-line selection.
- [x] 2.4 Implement the styles. `As typed` applies no case change.
- [x] 2.5 Failing tests for the strip options: punctuation and digits remove from within words only; `spaces: remove` deletes horizontal separators while the word boundary survives (`Hello World` + `Words Capitalized` → `HelloWorld`); `spaces: normalize` collapses runs and converts tabs, without trimming; `lines: join` yields one space and `lines: remove` yields none.
- [x] 2.6 Failing tests for composition and exclusivity: setting one member of a pair clears the other; `lines: join` + `spaces: normalize` produces exactly one space; identifier styles drop non-alphanumerics regardless of the options.
- [x] 2.7 Implement `Strips` as two three-valued settings plus two booleans, and the pipeline in the specified order: strip characters → resolve line breaks → resolve horizontal whitespace → apply style → join.
- [x] 2.8 Failing tests for `generate`: it returns **all** fourteen rows in declared order for every input, each carrying whether it would change the text; an all-uppercase selection still yields an `UPPERCASE` row, marked as changing nothing.
- [x] 2.9 Failing tests for the dim predicate: a toggle reports no effect when flipping it yields the same string for the selected style; with `spaces: remove` set, one of `lines: join`/`lines: remove` reports no effect; with no digits present, the digits toggle reports no effect.
- [x] 2.10 Implement `generate` and the dim predicate by rendering and comparing, with no per-style rules table.

## 3. Usage memory in the core, test-first

- [x] 3.1 Failing tests: recording increments a count; ordering is by count, highest first, with a stable tie-break; the top three are reported; `reset` clears everything; counts saturate rather than overflow.
- [x] 3.2 Failing tests for the text form: render then parse round-trips; an unknown style identifier in the text is ignored rather than rejected; malformed text parses as empty; empty text parses as empty.
- [x] 3.3 Implement the usage type. Text form only — no file IO in the core.

## 4. Configuration

- [x] 4.1 Failing tests: `hotkeys.change_case` defaults to `Win+Ctrl+Pause`; an empty value fails validation with the line reported; an existing file without the key still parses and gets the default.
- [x] 4.2 Implement the setting.
- [x] 4.3 Document `change_case` in `config.example.toml`, and note that usage counts live in a separate state file that deleting the configuration will not clear.

## 5. The case window

The five items left open below are *rendering* claims — group boundaries, dimming, the expanded
view, behaviour at 150% scale, the footer. The behaviour behind each is asserted by
`shell-checks.ps1 -Only Case`; what remains is whether it looks right, which needs a person.
They are written up as group D of `docs/testing/manual-checklist.md`.

- [x] 5.1 Create the window on the worker thread with its own modal loop, using the shared handshake from 1.1. `Esc` closes it and the document is untouched. Verify the tray stays responsive while it is open.
- [x] 5.2 Render the promoted section, the three groups with visible boundaries, and a preview per row using `preview::compact`. Mark rows that would change nothing.
      *Confirmed by manual group D, 2026-08-08: D1 groups and order, D2 dimming present and selectable, D6 every preview updates at once.*
- [x] 5.3 Render the toggle strip: two boolean toggles and two exclusive pairs on their own labelled rows, each showing its accelerator digit and its set state, with toggles that would change nothing marked.
      *Confirmed: D3 the three labelled rows read as pairs, D4 setting one member clears the other. D5 recorded as by-design — dim means "no effect on this style", not disabled.*
- [x] 5.4 Keys: arrows move between styles; `1`–`6` set the toggles without moving the selection; setting one member of a pair clears the other; every style's preview re-renders on a toggle.
- [x] 5.5 `Space` expands the selected style's result, scrollable from the keyboard, with omissions visible; expanding modifies nothing.
      *Confirmed: D7. The whole dialog blinked while scrolling; fixed by not asking Windows to erase the background before each repaint.*
- [x] 5.6 `Ctrl+R` discards the counts in place: the promoted section reverts to declared order and the window stays open.
- [x] 5.7 `Enter` commits; committing a row marked as changing nothing cancels instead of replacing text with itself. No layout switch is offered.
- [x] 5.8 Size the window to its content, and scroll the list keeping the selected row visible when the work-area clamp bites. Check at 100% and 150% scale.
      *Confirmed: D9 at 150%. The list scrolled with nothing on screen to say so; a proportional indicator was added.*
- [x] 5.9 Name every binding in the footer.
      *Confirmed: D8, every binding named.*

## 6. Wiring

- [x] 6.1 Add the state file path beside the configuration path, and the read and write in the binary. A missing, unreadable or malformed file yields no memory and no error.
- [x] 6.2 Add the case job to the worker. It owns the usage memory outright: loaded once, mutated in memory, written after a commit or a reset, never shared with the message-loop thread.
- [x] 6.3 Register both hotkeys and dispatch the case one. A conflict on either leaves the other working.
- [x] 6.4 Add the tray command for case conversion, mirroring the conversion command, so it works while the shortcut is held elsewhere.
- [x] 6.5 Report case outcomes through the existing channel: success, cancellation, selection unreadable, replacement failed — none reported as another.

## 7. Verification

- [x] 7.1 Prove `Win+Ctrl+Pause` registers on this machine. If it does not, choose another default and record why in `docs/manual-tests.md` — a default that ships in a conflict state is a defect, not a user problem.
- [x] 7.2 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`.
- [x] 7.3 `cargo build --release`; binary under 5 MB. Remember clippy's dev run does not surface release-only warnings.
- [x] 7.4 Add a Case group to `docs/testing/shell-checks.ps1`: the window opens; digits toggle; the other member of a pair replaces rather than stacks; `Ctrl+R` resets; `Esc` leaves the document untouched; the state file is written after an apply and not before; a conflict on one shortcut leaves the other working.
- [x] 7.5 Inspect the state file after several applies: style identifiers and integers only, no selected text, no result, no application name, no timestamp.
- [x] 7.6 Full `run-matrix.ps1` run — mandatory, because this touches `winui`. `-Smoke` is not sufficient.
- [x] 7.7 Manual: the promoted order does not change while the window is open and does reflect use on the next invocation; it survives a restart.
- [x] 7.8 Record results in `docs/manual-tests.md`, and update the architecture section of `CLAUDE.md` to name the second transaction.
