## 1. Startup stops being fatal

- [x] 1.1 Replace the `?` on hotkey registration at startup with a conflict state the program carries while running
- [x] 1.2 Replace the too-few-layouts exit with a disabled state that explains what is required
- [x] 1.3 Confirm by hand: start with the shortcut held by another application; the tray appears and the program keeps running
      *Verified 2026-08-07: a second instance started while the first held Shift+Pause kept
      running, logged `hotkey_conflict` naming the combination, and its tray icon received
      shell messages. Before this change the same run exited with code 1 and no icon.*
- [ ] 1.4 Confirm by hand: start with one layout installed; the program explains and stays reachable

## 2. Tray states

- [x] 2.1 Model the five states — enabled, disabled, too few layouts, shortcut conflict, runtime error
- [x] 2.2 Reflect the current state in the icon or tooltip, including the active shortcut where one is registered
- [x] 2.3 Add the conversion command, working whether or not the global shortcut is registered
      *Verified against a genuine conflict: a second instance, started while the first held
      Shift+Pause, converted `Ghbdsn ltoj` to `Привіт дещо` from the tray command. Its log
      reads `hotkey_conflict` then `selection_requested` then `outcome_applied`.*
- [ ] 2.4 Add settings and diagnostics entries
- [ ] 2.5 Confirm by hand that each of the five states is reachable and visibly different from the others

## 3. Shortcut recording

- [ ] 3.1 A focused window that captures a key combination without acting on it, reading keys as a focused window and never through a hook
- [ ] 3.2 Validate a recorded combination by attempting registration
- [ ] 3.3 Apply a valid combination without restarting, unregistering and re-registering as one operation
- [ ] 3.4 On failure, retain the previous working combination and report that the new one is unavailable
- [ ] 3.5 Persist the recorded shortcut to the configuration file, preserving comments and formatting elsewhere in it
- [ ] 3.6 Offer suggested alternatives, presented as suggestions rather than guarantees
- [ ] 3.7 **Milestone:** a conflict is resolved entirely from the keyboard, with no restart and no file editing

## 4. Configuration

- [ ] 4.1 Add the settings this change introduces — enabled state, and any preview sizing the chooser needs
- [ ] 4.2 Keep the file the complete description of behavior, with interface changes written back to it
- [ ] 4.3 Test: a shortcut changed through the interface appears in the file and survives a reload
- [ ] 4.4 Test: an invalid configuration still leaves the last known good in effect

## 5. Recovering without a restart

- [ ] 5.1 Become able to convert once a second layout is installed, without restarting
- [ ] 5.2 Clear the conflict state once a valid shortcut is registered
- [ ] 5.3 Confirm both transitions by hand

## 6. Verification — the shell's outstanding debt

Every item here covers something the previous implementation never exercised end to end.

- [ ] 6.1 Tray startup, and each menu command
- [ ] 6.2 Configuration reload applying a valid change while running
- [ ] 6.3 Configuration reload rejecting an invalid change and keeping the previous one
- [ ] 6.4 Shortcut registration, conflict, recording, re-registration and failure-to-rebind
- [ ] 6.5 Enable and disable actually gating conversion
- [ ] 6.6 Focus returns to the original application after every path, including the tray-invoked one
- [ ] 6.7 Release-build behaviour, not only debug — a clean `cargo clippy` does not surface release-only warnings
- [ ] 6.8 Record the results in `docs/manual-tests.md`, including anything that could not be covered and why
