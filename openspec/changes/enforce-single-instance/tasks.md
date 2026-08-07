## 1. The guard

- [x] 1.1 A single-instance guard in `lexiswitch-win`, acquired atomically and released when the process ends
- [x] 1.2 Treat access-denied as "already running", not as free
- [x] 1.3 Consult it before anything else in `main`, ahead of building layouts or taking the shortcut

## 2. What the second instance does

- [x] 2.1 Report that LexiSwitch is already running, rather than exiting silently
- [x] 2.2 Ask the running instance to make itself visible, by posting to its message window
- [x] 2.3 The running instance answers with a balloon naming itself and its shortcut
- [x] 2.4 Exit `0`: the user wanted it running and it is
- [x] 2.5 Decline anyway when the running instance cannot be reached, and still say why

## 3. Adapt what depended on two instances

- [x] 3.1 A probe that holds a hotkey combination, standing in for a third-party application
- [x] 3.2 Repoint the conflict, recording and recovery harnesses at it
- [x] 3.3 Restate manual checklist item B3, which starts a second instance to reach the conflict state

## 4. Verification

- [x] 4.1 A second launch declines, and the first is untouched — shortcut, configuration and tray icon
- [x] 4.2 The running instance shows itself when a second launch is attempted
- [x] 4.3 Two instances started together: exactly one runs
- [x] 4.4 A start after the previous instance was killed succeeds, proving nothing is left behind
- [x] 4.5 A genuine conflict with another application still reports and still recovers
- [x] 4.6 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, release build under 5 MB
- [x] 4.7 Compatibility matrix, full run
- [x] 4.8 Record the results in `docs/manual-tests.md`
