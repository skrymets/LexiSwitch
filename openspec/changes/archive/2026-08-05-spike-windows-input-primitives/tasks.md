## 1. Scaffold

- [x] 1.1 Create `spikes/` with a `README.md` stating that nothing here is a workspace member, nothing here ships, and quality rules do not apply
- [x] 1.2 Add `spikes/*/target` to `.gitignore` (already present — verify it covers the layout actually used)
- [x] 1.3 Create `docs/spikes/2026-08-05-windows-input-primitives.md` with the machine configuration: Windows build number, installed keyboard layouts as reported by Windows Settings, and whether the session is elevated

## 2. Spike 3 — layout table extraction (run first; blocks the rest)

- [x] 2.1 Create `spikes/spike3_layout_tables/` as a standalone cargo project depending on `windows` with `Win32_UI_Input_KeyboardAndMouse` and `Win32_UI_TextServices`
- [x] 2.2 Enumerate installed layouts via `GetKeyboardLayoutList`; print each HKL with its language identifier and resolved name
- [x] 2.3 Probe one layout across the alphanumeric and punctuation scancode range in unshifted and shifted states via `ToUnicodeEx`, printing `(scancode, shift) → char`
- [x] 2.4 Implement dead-key state flushing between probes; classify any position `ToUnicodeEx` reports as dead as dead rather than storing a resolved character
- [x] 2.5 **Prove the flush works:** deliberately probe a German dead key (`^`, `´`), then probe a known key immediately after, and confirm the second result is unaffected. A spike that does not test its own flush proves nothing
- [x] 2.6 Emit JSON tables for uk, en-US, and de to stdout; eyeball each against a physical keyboard, key by key
- [x] 2.7 Write the JSON to `lexiswitch-core/tests/fixtures/{uk,en_us,de}.json` and commit as frozen fixtures
- [x] 2.8 Record in the findings doc: whether extraction succeeded, how dead keys behave, which `windows` crate features were actually needed
- [x] 2.9 **Decision gate:** if extraction is not viable, stop and write an ADR superseding ADR-0004 before any further work. `add-manual-layout-conversion` is blocked until then

## 3. Spike 1 — UIPI reach

- [x] 3.1 Create `spikes/spike1_uipi_reach/` as a standalone cargo project depending on `windows` with `Win32_UI_Input`, `Win32_UI_WindowsAndMessaging`
- [x] 3.2 Create a hidden message-only window and register for keyboard Raw Input with `RIDEV_INPUTSINK`
- [x] 3.3 Print each `WM_INPUT` keyboard event as `(scancode, vkey, flags)` alongside the foreground window's process name
- [x] 3.4 Run unelevated; type into Notepad and confirm events appear
- [x] 3.5 Launch an elevated Notepad or Windows Terminal, type into it, and record whether events appear — this is the question
- [x] 3.6 Record the answer in the findings doc. If elevated windows are opaque, add the limitation to ADR-0003 as a consequence and note the workaround (run LexiSwitch elevated) with its trade-off

## 4. Spike 2 — hotkey timing

- [x] 4.1 Create `spikes/spike2_hotkey_timing/` as a standalone cargo project depending on `windows` with `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_WindowsAndMessaging`
- [x] 4.2 Register `Pause` via `RegisterHotKey` against the message-only window
- [x] 4.3 On each `WM_HOTKEY`, print the message timestamp and the delta from the previous one — read the timestamp from the message, not from a wall-clock call inside the handler
- [x] 4.4 Tap deliberately at single, double, and triple speeds; record the observed deltas and the timestamp granularity
- [x] 4.5 Test the conflict path: have another application hold `Pause`, confirm `RegisterHotKey` fails, and record what the failure looks like — the error surfacing in ADR-0008 depends on this being detectable
- [x] 4.6 Record in the findings doc whether 500 ms is a workable default for ADR-0009's multi-tap window, or what value the observed granularity suggests instead

## 5. Close out

- [x] 5.1 Complete `docs/spikes/2026-08-05-windows-input-primitives.md`: one section per spike, each stating the question, what was observed, and the answer
- [x] 5.2 Consolidate the `windows` crate feature set the three spikes actually needed; record it for `lexiswitch-win` to inherit
- [x] 5.3 Write any superseding ADR a spike result requires. Do not amend an existing ADR in place
- [x] 5.4 Confirm `cargo build` and `cargo test --workspace` at the repo root neither compile nor run anything under `spikes/`
- [x] 5.5 Update `add-manual-layout-conversion` if a finding changes its design or tasks
