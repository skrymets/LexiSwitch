## 1. Retire the consumers

Leaf-first, so the tree stays green and every deletion is obviously intended rather than
fallout from an earlier one.

- [x] 1.1 Remove the `ConvertLastWord`, `ConvertClipboard` and `PauseToggle` hotkey arms and the `convert_last_word` helper from `lexiswitch/src/main.rs`
- [x] 1.2 Remove `Job::Replace` and `Job::ClipboardOnly` and the `convert_clipboard` function from `lexiswitch/src/worker.rs`
- [x] 1.3 Remove the `WM_INPUT` arm and the buffer, gate, tap and correction state from the message loop
- [x] 1.4 Build and run the workspace; selection conversion still works end to end

## 2. Retire capture and cycling

- [x] 2.1 Delete `lexiswitch-win/src/rawinput.rs` and its module declaration
- [x] 2.2 Delete `lexiswitch-win/src/pagelock.rs` and its module declaration
- [x] 2.3 Delete `lexiswitch-core/src/buffer.rs` and `lexiswitch-core/src/cycle.rs` and their module declarations
- [x] 2.4 Delete `lexiswitch-core/tests/{buffer,cycling,extraction,invalidation}.rs`
- [x] 2.5 Remove `PasswordGate`, `inspect`, `FocusInfo` and `Detection` from `lexiswitch-win/src/focus.rs`, keeping `Uia` and `process_name`
- [x] 2.6 Remove `inject::replace` and `SYNTHETIC_MARKER` from `lexiswitch-win/src/inject.rs`, keeping `paste`, `copy_selection` and `FocusGuard`
- [x] 2.7 Update the four `examples/probe_*.rs` that use `PasswordGate` as a foreground-process helper to use `process_name` instead
- [x] 2.8 Delete `lexiswitch-win/examples/probe_focus.rs`, whose only purpose was measuring password-field detection
- [x] 2.9 `cargo clippy --workspace --all-targets` clean; conversion tests unchanged and green

## 3. Configuration

Test-first: `lexiswitch-core` config work is logic and must be covered without Windows.

- [ ] 3.1 Write a failing test: a config file containing removed keys loads, applies the keys that remain, and reports the ignored ones by name
- [ ] 3.2 Relax `deny_unknown_fields` on the top-level config and collect unrecognised keys
- [ ] 3.3 Remove `convert_last_word`, `convert_clipboard`, `pause_toggle` and `multi_tap_window_ms` from the hotkey settings
- [ ] 3.4 Remove `BufferConfig` and `Exclusions` entirely, and `behavior.switch_layout_after_correction`
- [ ] 3.5 Report ignored keys at startup through the tray, distinguishing them from a parse error
- [ ] 3.6 Update `config.example.toml` to the reduced surface
- [ ] 3.7 Write a test proving a current-version config file still loads after the change

## 4. Tray

- [ ] 4.1 Rename `MenuCommand::PauseResume` to enable/disable and update its labels
- [ ] 4.2 Make the disabled state stop the hotkey converting, without reference to capture
- [ ] 4.3 Confirm by hand: disable, press the hotkey, nothing converts; enable, it converts again

## 5. Prove the absence

- [x] 5.1 `cargo build --release`; confirm the binary stays under 5 MB
- [x] 5.2 Search the release binary for `RegisterRawInputDevices`, `RIDEV` and `SetWindowsHookEx`; none may appear
- [x] 5.3 Record the result in `docs/manual-tests.md` as the evidence for the pivot's no-observation invariant

## 6. Verification

- [x] 6.1 `cargo test --workspace` green, with only the deleted suites missing
- [ ] 6.2 `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`
- [ ] 6.3 Run for an hour idle; confirm resident memory under 20 MB (migrated from `add-manual-layout-conversion` 10.4)
- [ ] 6.4 Monitor network activity during an extended session; confirm no outbound connection (migrated from 10.5)
- [ ] 6.5 Rewrite the manual checklist for the reduced product and execute it (migrated from 10.7)
- [ ] 6.6 **Experiment:** run the release build and check the CrowdStrike event log. Record the outcome either way — a still-quarantined binary means the block was never attributable to capture, and the pivot's endpoint-protection criterion needs rethinking rather than retrying

## 7. Close out the superseded change

- [ ] 7.1 Populate `openspec/specs/` from `add-manual-layout-conversion` so these deltas have a base to apply against — the previous change was never synced, so main specs are empty
- [ ] 7.2 Drop task 7.8 of that change, which asks for a capture exclusion list that no longer exists
- [ ] 7.3 Note that its task 8.6 moves to the chooser change rather than being archived as unfinished
- [ ] 7.4 Archive `add-manual-layout-conversion`
