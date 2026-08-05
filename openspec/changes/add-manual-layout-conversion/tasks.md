## 1. Workspace and gates

- [x] 1.1 Create the workspace `Cargo.toml` with members `lexiswitch-core`, `lexiswitch-win`, `lexiswitch`; set release profile to LTO and `panic = "abort"`
- [x] 1.2 Create `lexiswitch-core` with `#![forbid(unsafe_code)]` and no dependency that can reach Win32
- [x] 1.3 Write `deny.toml` denying network-capable crates (`reqwest`, `hyper`, `curl`, `ureq`, `tokio` net features)
- [x] 1.4 Verify the gate works: temporarily add a networking dependency, confirm `cargo deny check` fails, then remove it. An unproven gate is not a gate
- [x] 1.5 Confirm the frozen layout fixtures from `spike-windows-input-primitives` are present at `lexiswitch-core/tests/fixtures/{uk,en_us,de}.json`

## 2. Conversion — `lexiswitch-core`, test-first

- [x] 2.1 Write failing tests for loading a `LayoutTable` from fixture JSON, including dead-key positions
- [x] 2.2 Implement `LayoutTable` with forward and reverse maps; make 2.1 pass
- [x] 2.3 Write failing tests for position-based conversion: `ауфегкуі` → `features`, mixed case preserved, QWERTZ `y`/`z` transposition
- [x] 2.4 Implement conversion; make 2.3 pass
- [x] 2.5 Write failing tests for pass-through: characters absent from the source layout, and dead-key positions in either layout, are returned unchanged
- [x] 2.6 Implement pass-through; make 2.5 pass
- [x] 2.7 Write a failing test that conversion producing no change is reported as no change, distinct from a successful identical conversion
- [x] 2.8 Implement the no-change signal; make 2.7 pass
- [x] 2.9 Add a property test: round trip A→B→A is identity for text mappable in both layouts
- [x] 2.10 Add the regression corpus: German umlauts, shifted punctuation, digits, mixed-script strings
- [x] 2.11 **Milestone:** `cargo test -p lexiswitch-core` passes on a machine with no Windows APIs involved

## 3. Buffer and cycling — `lexiswitch-core`, test-first

- [x] 3.1 Write failing tests for the 64-entry ring: oldest entries discarded, capacity never exceeded
- [x] 3.2 Implement `KeyBuffer` storing `(scancode, shift_state, tick)`; make 3.1 pass
- [x] 3.3 Write one failing test per invalidation trigger: focus change, `Enter`, `Tab`, `Escape`, idle timeout, successful conversion, pause
- [x] 3.4 Implement invalidation; make 3.3 pass
- [x] 3.5 Implement zeroize-on-wipe and zeroize-on-drop; add a test asserting the backing storage is cleared after a wipe
- [x] 3.6 Write failing tests for word extraction from buffer contents, including the empty and stale cases
- [x] 3.7 Implement word extraction, resolving key positions to text via a layout table at extraction time; make 3.6 pass
- [x] 3.8 Write failing tests for cycling: two-layout wrap (second press restores original), three-layout wrap, each press rendering from the retained original rather than the previous result
- [x] 3.9 Write failing tests for multi-tap window boundaries: within the window continues the cycle, beyond it starts a new correction
- [x] 3.10 Implement cycling; make 3.8 and 3.9 pass

## 4. Configuration — `lexiswitch-core`, test-first

- [x] 4.1 Write failing tests for parsing `config.example.toml` and for each default value
- [x] 4.2 Implement the config types and defaults; make 4.1 pass
- [x] 4.3 Write a failing test that disabling password-field protection is rejected by validation
- [x] 4.4 Implement validation, including line-referenced error messages; make 4.3 pass
- [x] 4.5 Write failing tests for exclusion matching by process name; implement and make them pass
- [x] 4.6 Write `config.example.toml` covering every setting with a comment explaining each

## 5. Observation — `lexiswitch-win`

- [x] 5.1 Create the crate; add `windows` with the feature set established by the spike
- [x] 5.2 Create a hidden message-only window and its message loop
- [x] 5.3 Register keyboard Raw Input with `RIDEV_INPUTSINK`; parse `WM_INPUT` into `(scancode, vkey, flags, up/down)`, discarding modifier-key events and collapsing auto-repeat — the spike observed 14 indistinguishable `down` events from one held key, enough to evict real typing from a 64-entry ring
- [x] 5.4 Register hotkeys via `RegisterHotKey`; on failure, report which combination is unavailable and abort startup
- [x] 5.5 Classify multi-tap from `WM_HOTKEY` message timestamps, using the window value the spike validated
- [x] 5.6 Build layout tables at startup from `GetKeyboardLayoutList` and `ToUnicodeEx`, flushing dead-key state between probes; refuse to start with fewer than two layouts
- [x] 5.7 Write `examples/dump_layouts.rs` printing every derived table; confirm output matches the frozen fixtures
- [x] 5.8 **Milestone:** running the binary builds layout tables for every installed layout, registers the hotkeys without conflict, and shows a tray icon

## 6. Replacement — `lexiswitch-win`

- [x] 6.1 Implement injection: `SendInput` backspaces followed by Unicode characters
- [x] 6.2 Verify the foreground window is unchanged immediately before injecting; abandon the operation if focus moved
- [x] 6.3 Implement post-correction layout activation for the foreground window
- [x] 6.4 **Milestone:** typing `ауфегкуі` in Notepad and pressing the hotkey yields `features` with the layout switched
- [x] 6.5 **Milestone:** the press following the last candidate restores `ауфегкуі` exactly — with two layouts installed that is the second press, with three the third

## 7. Containment — `lexiswitch-win`

- [x] 7.1 Implement foreground-window inspection: window handle to process id to process name
- [x] 7.2 Implement password-field detection via the accessibility `IsPassword` property, falling back to the `ES_PASSWORD` window style
- [x] 7.3 Write `examples/probe_focus.rs` reporting foreground process and password-field state
- [x] 7.4 **Measure detection coverage** with `probe_focus` across Chrome, Firefox, Electron, Windows Terminal, VS Code, and a password manager; record results in `docs/manual-tests.md`
- [x] 7.5 Gate buffer filling on the detection result, the exclusion list, and pause state
- [x] 7.6 Implement `VirtualLock` on the buffer's backing pages
- [x] 7.7 **Milestone:** focusing a password field, typing, and pressing the hotkey is a no-op, and `probe_focus` confirms the buffer never filled
- [ ] 7.8 If 7.4 shows gaps in any application, add it to the shipped default exclusion list in `config.example.toml`

## 8. Selection and clipboard — `lexiswitch-win`

- [x] 8.1 Implement reading and replacing the selection through the accessibility text pattern; wrap COM interfaces in RAII types at the module boundary so no raw pointer escapes
- [x] 8.2 Implement the clipboard fallback: snapshot text formats, copy, convert, paste, restore
- [x] 8.3 Implement retry with backoff on clipboard acquisition failure; after retries, abandon **without pasting** and report
- [x] 8.4 Honour the configuration switch disabling the clipboard fallback; report unsupported rather than falling back
- [x] 8.5 Implement clipboard-only conversion: read, convert, write back, paste nothing
- [ ] 8.6 **Milestone:** selection conversion works in VS Code (automation path) and Windows Terminal (fallback path), with the prior clipboard restored in both

## 9. Agent shell — `lexiswitch`

- [x] 9.1 Wire startup: load config, build layout tables, register hotkeys, create the window, start the worker thread
- [x] 9.2 Implement the two-thread split — buffer owned by the message-loop thread, snapshots sent to the worker over a channel, no lock on the buffer
- [x] 9.3 Implement the tray icon with distinct active and paused appearances
- [x] 9.4 Implement the tray menu: pause/resume, reload config, open config, quit
- [x] 9.5 Implement config hot-reload by polling mtime every 2 seconds; swap validated configuration wholesale, keep last-good on failure
- [x] 9.6 Route every failure path to a tray notification stating what did not happen
- [x] 9.7 Implement opt-in logging recording event kinds and lengths only; default off
- [x] 9.8 Implement clean shutdown: release hotkeys, wipe the buffer, exit with no process remaining

## 10. Verification

- [x] 10.1 `cargo test --workspace` passes
- [x] 10.2 `cargo deny check` and `cargo audit` pass
- [x] 10.3 `cargo build --release`; confirm binary under 5 MB
- [ ] 10.4 Run for an hour idle; confirm resident memory under 20 MB
- [ ] 10.5 Monitor network activity during an extended session; confirm no outbound connection
- [x] 10.6 Write `docs/manual-tests.md` as a runnable checklist covering Notepad, Word, Chrome, Windows Terminal, VS Code, Telegram
- [ ] 10.7 Execute the manual checklist and record results
- [x] 10.8 Verify with logging enabled that no log line contains converted text
- [x] 10.9 Confirm every `unsafe` block in `lexiswitch-win` carries a comment stating its invariant
