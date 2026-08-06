# Manual tests

Behaviour that cannot be covered by automated tests. The checklist below is meant to be
executed; everything after it records what was observed when it was, as observations rather
than claims.

Machine configuration is in [`environment.md`](environment.md). Unless stated otherwise,
LexiSwitch ran unelevated.

---

## The checklist

Run the release build unless a step says otherwise. Record the date and the outcome of every
step — including the ones that pass, because "we ran it and it worked" and "we never got to
it" are indistinguishable in an empty results section.

```powershell
cargo build --release
.\target\release\lexiswitch.exe
```

### Setup

- [ ] Two or more keyboard layouts installed; the program refuses to start otherwise.
- [ ] Tray icon appears, and its tooltip reads `LexiSwitch`.
- [ ] Startup balloon names the hotkey for last-word correction.
- [ ] No console window appears (release build only).

### Last-word correction — one row per application

For each: type the `test` keys with the **Ukrainian** layout active so the screen shows
`еуіе`, press the last-word hotkey, and confirm it becomes `test`. Then press it again
within 500 ms and confirm the cycle advances.

| Application                           | Converts | Cycles | Notes                                                                           |
|---------------------------------------|----------|--------|---------------------------------------------------------------------------------|
| Notepad                               | ☐       | ☐     | Native baseline; everything should work                                         |
| Word                                  | ☐       | ☐     | Rich text, autocorrect may interfere                                            |
| Chrome (address bar and a text field) | ☐       | ☐     |                                                                                 |
| VS Code                               | ☐       | ☐     | Electron with good accessibility                                                |
| Windows Terminal                      | ☐       | ☐     | **Excluded by default** — expect a no-op unless removed from the exclusion list |
| Telegram                              | ☐       | ☐     | Electron chat client                                                            |

- [ ] Pressing the hotkey with nothing recently typed shows "Nothing to correct" rather than
      doing something arbitrary.
- [ ] Pressing the hotkey after 30 s of not typing does the same — the buffer has gone stale.
- [ ] Typing a word, pressing Enter, then pressing the hotkey is a no-op. Enter wipes.
- [ ] **Do not press another key immediately after the hotkey.** A keystroke landing within
      ~300 ms can be processed inside the injected sequence — see the race section below.

### Selection conversion (task 8.6)

- [ ] **VS Code** — select mistyped text, press the selection hotkey. Converts in place, and
      the clipboard still holds whatever it held before.
- [ ] **Windows Terminal** — same, exercising the clipboard fallback. Clipboard restored.
- [ ] With an **image** on the clipboard, run the fallback path. The image is lost, and the
      tray says so. (Expected: text-only restore is deliberate.)
- [ ] Set `allow_clipboard_fallback = false`, retry in Windows Terminal. Reports
      "does not expose its selection" rather than silently using the clipboard.

### Clipboard conversion

- [ ] Copy mistyped text, press the clipboard hotkey. The clipboard now holds the converted
      text, and **nothing was pasted anywhere**.
- [ ] With a non-text clipboard, the same hotkey reports "holds no text".

### Containment (section 7)

- [ ] Focus a password field in Chrome, type, press the hotkey — no-op, nothing recorded.
- [ ] Same in a password manager (listed in the default exclusions).
- [ ] Same in Windows Terminal at a `sudo` prompt — protected by the exclusion list, since
      detection cannot see it.
- [ ] `probe_focus` reports `YES via UIAutomation` for each field above where possible.
- [ ] Pause via the tray, type, resume: nothing typed while paused is recoverable.

### The agent (section 9)

- [ ] Tray menu opens on right-click and offers Pause, Reload, Open configuration, Quit.
- [ ] Icon changes visibly between active and paused.
- [ ] "Open configuration file" creates `%APPDATA%\LexiSwitch\config.toml` when absent and
      opens it.
- [ ] Editing and saving the file applies within ~2 s, with a confirming balloon.
- [ ] Saving a file with `respect_password_fields = false` is **refused**, names the line,
      and the previous settings stay in effect.
- [ ] Saving a syntax error does the same.
- [ ] Changing a hotkey re-registers it; the old binding stops working, the new one works.
- [ ] Quit from the tray menu leaves no `lexiswitch.exe` in Task Manager, and the hotkeys
      are free for another application to claim.

### Budgets

- [ ] Idle for an hour: resident memory under 20 MB (`Get-Process lexiswitch`).
- [ ] No outbound connection during an extended session (`resmon`, filtered to the process).

---

## Password-field detection coverage (task 7.4)

**Measured 2026-08-05** with `cargo run -p lexiswitch-win --example probe_focus`, clicking
into a password field in each application and reading what the probe reported.

This is the measurement the runtime privacy model depends on
([ADR-0005](adr/0005-bounded-in-memory-keystroke-buffer.md)). It is taken, not assumed, and
the result is that **detection is good in browsers and Office, and fails in terminals and
JetBrains IDEs.**

| Application                      | Reported | Via                         | Correct?                  |
|----------------------------------|----------|-----------------------------|---------------------------|
| Chrome                           | `YES`    | UI Automation               | ✅                        |
| Word                             | `YES`    | UI Automation               | ✅                        |
| Windows Terminal (`sudo` prompt) | `no`     | UI Automation               | ❌ **missed**             |
| PyCharm (proxy settings)         | `no`     | UI Automation               | ❌ **missed**             |
| RustRover (proxy settings)       | `no`     | UI Automation               | ❌ **missed**             |
| SumatraPDF (ordinary control)    | `no`     | `ES_PASSWORD`, then nothing | ✅ (not a password field) |

Not yet measured: Firefox, VS Code, an Electron chat client, and a password manager. The
password managers ship in the default exclusion list, so a detection failure there is already
contained — but the measurement is still worth taking, because the exclusion list is a
backstop and not a substitute.

### Chrome detects correctly, which is better than expected

ADR-0005 anticipated that "inside Chrome, Firefox, and Electron applications, whether the
focused element reports `IsPassword` depends on the app's accessibility implementation".
Chrome's implementation answers correctly. That is one of the three named risk applications
cleared by measurement.

### Windows Terminal cannot be fixed, only avoided

A terminal password prompt is not a password field. There is no control with an `IsPassword`
property — there is a grid of text and a shell that has stopped echoing. `[sudo] Enter
password for coder:` looks identical to any other line to every accessibility API, because
to the terminal it *is* identical.

No detection improvement will close this. The only mechanisms that can are the exclusion
list, the pause hotkey, and the buffer's short invalidation timeouts.

This one stings: ADR-0005 lists terminals among the places last-word correction is most
wanted, precisely because selection-based approaches are unreliable there. Excluding
Windows Terminal trades a feature the design wanted for a risk it cannot otherwise bound.

**Decision: `WindowsTerminal.exe` is in the shipped exclusion list** (task 7.8). Correction
does not work in Windows Terminal by default. For a session with no password prompt in it,
the pause-toggle hotkey is the intended escape hatch, or the entry can be removed from
`config.toml` — the cost of doing so is now written down rather than discovered.

### JetBrains IDEs may be fixable at the application end

PyCharm and RustRover are Java applications whose accessibility surface is off unless
enabled. Their password fields report nothing to UI Automation as shipped, which is why the
probe answers `no` with apparent confidence.

**Decision: not excluded yet, pending one more measurement.** Excluding an IDE disables
correction in the *editor* as well as the settings dialogs where the password fields
actually are, which is a large cost for a narrow risk — and the risk may not be inherent.

Test whether **Settings → Appearance & Behavior → Accessibility → "Support screen
readers"** makes `IsPassword` visible to `probe_focus`:

- If it does, the gap is a configuration issue. Document the setting; no exclusion needed.
- If it does not, add `rustrover64.exe`, `pycharm64.exe` and their siblings
  (`idea64.exe`, `webstorm64.exe`, `clion64.exe`, `goland64.exe`, `phpstorm64.exe`,
  `rider64.exe`, `datagrip64.exe`) to the exclusion list — they share an architecture, so a
  failure in one predicts the rest.

_Status: untested. Until it is resolved, typing a password into a JetBrains settings dialog
is recorded into the buffer, bounded only by the 64-entry cap, the idle timeout, and the
pause control._

### What a `no` is worth, by route

- `via UIAutomation` — the application implements accessibility and answered. Trustworthy.
- `via ES_PASSWORD` — only meaningful for native Win32 controls. A `no` here says little
  about a browser, an Electron app, or a custom-drawn control.
- `via none` — nothing could answer. Treated as not-a-password, which is the permissive
  direction, and the reason the exclusion list exists.

---

## Correction in real applications (tasks 6.4, 6.5)

**Measured 2026-08-05**, Windows 11 Notepad, Ukrainian and en-US installed.

- **Task 6.4 — passes.** Typing the `test` scancodes under Ukrainian shows `еуіе`; pressing
  the hotkey replaces it with `test`. Verified manually and by an automated harness that
  drives Notepad and reads the result back through the clipboard: 12/12 clean when no
  further key is pressed.

- **Task 6.5 — cannot pass as written with three layouts installed.** The task expects the
  second press to restore the original. With uk-UA, en-US and de-DE installed the cycle has
  three stops, and `test` renders identically under en-US and German, so the second press is
  a no-op and the *third* restores. Observed:

  ```
  press=1  rendered="test"  -> injected
  press=2  rendered="test"  -> no change
  press=3  rendered="еуіе"  -> injected, original restored
  ```

  This is [ADR-0009](adr/0009-multi-tap-layout-cycling.md)'s general rule behaving correctly;
  the milestone's wording assumes the two-layout case.

### Injected text races the next keystroke

A keystroke pressed immediately after the hotkey can be processed *inside* the injected
sequence. Observed as `еуі⏎е` — the Enter landed between the third and fourth injected
character.

Measured by pressing Enter at varying delays after the hotkey:

| Gap after hotkey | Runs intact |
|------------------|-------------|
| 0 ms             | 3/5         |
| 40 ms            | 4/5         |
| 300 ms           | 5/5         |
| 1200 ms          | 6/6         |

The sequence is already minimal — four backspaces and four characters in a single
`SendInput` call — and this is the hazard the change's design names as accepted:
*"Synthetic input can race the target application… This cannot be fully eliminated with
synthetic input."*

**`BlockInput` was considered and rejected.** Suppressing physical input during injection
would close the race completely. It would also give LexiSwitch the ability to block typing —
exactly the capability the README says it does not have, and the reason
[ADR-0003](adr/0003-raw-input-over-low-level-hook.md) chose Raw Input over a hook. A 300 ms
convenience is not worth the project's central security claim.

The automation path in [ADR-0006](adr/0006-uia-first-clipboard-fallback.md) replaces text
without synthetic input at all. It is currently planned only for selections (task 8.1);
extending it to last-word replacement would sidestep this race in applications that support
it.

---

## Verification suite (section 10)

**Run 2026-08-05.**

| Task                              | Result                                                   |
|-----------------------------------|----------------------------------------------------------|
| 10.1 `cargo test --workspace`     | 108 passed, 0 failed                                     |
| 10.2 `cargo deny --offline check` | `advisories ok, bans ok, licenses ok, sources ok`        |
| 10.2 `cargo audit -n`             | 37 dependencies scanned, no vulnerabilities              |
| 10.3 release binary under 5 MB    | **578 KB**, 11.3% of budget                              |
| 10.8 no content in logs           | pass — see below                                         |
| 10.9 every `unsafe` documented    | 46/46, and both other crates carry `forbid(unsafe_code)` |

### 10.8 — logging records sizes, never content

With `logging.enabled = true`, the complete log from a real correction of `еуіе` → `test`:

```
layouts_built count=3
word_extracted len=4
correction_sent len=4
```

Asserted absent: the source text, the converted text, and any Cyrillic character at all.

This is structural rather than careful. `EventLog::event(kind, len)` takes a **length, not a
string** — there is no parameter through which text could be passed, so a caller cannot leak
content by mistake.

Caveat: run against the debug build (see below), with `LEXISWITCH_DIAG` unset. The
debug-only diagnostic scaffolding from the section 6 investigation *does* print content when
that variable is set; it is compiled out of release builds entirely and is removed once the
outstanding milestones pass.

### 10.5 — supporting static evidence

Not the runtime observation the task asks for, which still needs an extended session under a
network monitor. But the shipped binary's import table names every DLL it can call into, and
**no networking library appears**:

```
api-ms-win-core-synch, api-ms-win-crt-*, bcryptprimitives, dbghelp, kernel32,
ntdll, ole32, oleaut32, shell32, user32, vcruntime140
```

Absent: `ws2_32`, `wsock32`, `wininet`, `winhttp`, `urlmon`, `dnsapi`, `mswsock`. A program
cannot open a socket without one of these. That is a stronger claim than watching traffic for
an hour, because it covers every code path rather than the ones that happened to run —
though it does not cover a library resolving a networking entry point dynamically, which is
what the runtime check is still for.

### The release build is blocked; the debug build is not

**Observed 2026-08-05.** Launching `target\release\lexiswitch.exe` fails with *"Access is
denied"*, while `target\debug\lexiswitch.exe` from the same directory starts normally. The
file is intact (readable, valid `MZ` header), the ACLs grant `ReadAndExecute` to `Users`,
and there is no mark-of-the-web stream — so this is not a permissions problem.

The release build presents a more suspicious profile than the debug one: optimised, symbols
stripped, no console (`windows_subsystem = "windows"`), registering global hotkeys and
calling `SendInput`.

Release-build verification cannot be run on this machine until this is resolved, which
affects 10.4 (an hour resident) and 10.5 (network observation) as well as any release-mode
manual testing.

> **Correction, 2026-08-07.** The original entry attributed this to Avast and concluded it
> was "not a quarantine problem". Both are wrong, and the entry above has been amended to
> remove the attribution while leaving the observation itself intact.
>
> **CrowdStrike Falcon is installed and is what acts**, confirmed from its own event log.
> Two events are recorded, with no further detail in either:
>
> ```
> A process was blocked because malicious behavior was detected.
> A file was quarantined because malicious behavior was detected.
> ```
>
> Three things follow. It *is* a quarantine, so the original inference from the intact file
> was unsound — the file was intact when inspected, not when blocked. The verdict is
> CrowdStrike's generic behavioural ML result rather than a named rule, so **nothing here
> attributes the block to any particular thing the program does**. And a *file*-level
> quarantine of an unsigned, zero-prevalence binary points at reputation as much as at
> behaviour.
>
> That last point matters for the pivot. The brief's §22 treats "release build no longer
> blocked by endpoint protection" as an outcome of removing keystroke capture, but the
> evidence does not support attributing the block to capture, and every other suspicious
> attribute above survives the pivot. Treat it as an experiment to run after capture is
> removed, not as a benefit to assume — and note that code signing, currently deferred in
> [`environment.md`](environment.md) §6 on the grounds that the binary is not distributed,
> is a plausible cause that the pivot does not touch.

### The release build now runs; the block does not reproduce

**Observed 2026-08-07**, as the pre-removal baseline for the pivot's endpoint-protection
experiment. `target\release\lexiswitch.exe` launches, stays running, and is not quarantined.
Working set at startup is **13.1 MB**, against the 20 MB budget.

The 2026-08-05 *"Access is denied"* does not reproduce. Three explanations fit and this
observation cannot separate them: CrowdStrike's policy or exclusions changed in the interim,
the binary was rebuilt and the new one carries a different verdict, or the original detection
was specific to that build. Nothing here says the block will not return.

**A first attempt did fail, for an unrelated reason worth recording separately.** The release
build exited with code 1, and the debug build — which has a console — named the cause:

```
lexiswitch: hotkey convert_last_word could not be registered: Hot key is already registered.
```

A debug instance left running from the previous day held `Pause`. Once it was stopped, the
release build started normally. Two things follow. Exit code 1 at startup is not evidence of
endpoint protection, and anyone testing this must check for a stale instance first. And this
is the fatal-hotkey-conflict behaviour demonstrated live rather than argued: a combination held
by *anything*, including a forgotten copy of the program itself, ends the process with no
window, no tray icon, and no way to choose another. That is the condition
`rework-shortcut-and-shell` exists to remove.

**Consequence for the pivot.** §22's criterion — "the release build is not blocked because of
typing-monitor behaviour in the tested endpoint-protection environment" — currently has no
failing baseline to improve on. Removing keystroke capture cannot be shown to fix something
that is not presently broken. Re-test after the removal to confirm it stays unblocked, but do
not expect the removal to demonstrate a benefit.

---

## Elevated windows (spike 1)

Recorded in full in [`spikes/2026-08-05-windows-input-primitives.md`](spikes/2026-08-05-windows-input-primitives.md).
Keystrokes typed into an elevated window are not delivered to an unelevated LexiSwitch at
all, so correction silently does nothing there. Accepted limitation, noted in ADR-0003.
