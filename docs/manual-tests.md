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

Rewritten 2026-08-07 for the reduced product. Steps covering last-word correction, clipboard
conversion, multi-tap cycling, password-field detection, the exclusion list and pause/resume
are gone because the features are ([ADR-0011](adr/0011-no-keyboard-observation.md),
[ADR-0012](adr/0012-single-chooser-transaction.md)). What those steps *found* is kept below,
under its original dates — an observation stays true even when the thing it observed has been
removed.

```powershell
cargo build --release
.	arget
elease\lexiswitch.exe
```

### Setup

- [ ] Two or more keyboard layouts installed.
- [ ] Tray icon appears, and its tooltip reads `LexiSwitch`.
- [ ] Startup balloon names the selection hotkey.
- [ ] No console window appears (release build only).
- [ ] **Check for a stale instance first.** A hotkey held by an earlier copy of the program
      makes registration fail, which ends startup — see the 2026-08-07 note below. Confirm no
      `lexiswitch.exe` is already running before concluding anything about a failure to start.

### Selection conversion

- [ ] **Notepad** — select mistyped text, press the selection hotkey. Converts in place.
- [ ] The clipboard still holds whatever it held before the conversion.
- [ ] **One** `Ctrl+Z` reverts the whole conversion, not one character of it.
- [ ] **Notepad++** — same, exercising the clipboard read path (it exposes no text pattern).
- [ ] **A browser text field** — same. Run it twice: the read path may differ between runs,
      and both must work.
- [ ] With an **image** on the clipboard, convert a selection. The image is lost and the tray
      says so. Text-only restore is deliberate.
- [ ] With plain text on the clipboard, convert a selection. The tray stays **silent** — the
      notice fires only when something was actually destroyed.
- [ ] Set `allow_clipboard_fallback = false`, retry where no text pattern exists. Reports
      "does not expose its selection" rather than silently using the clipboard.

### Configuration migration

- [ ] Start with a config file from the previous version. The program **starts**, and a
      balloon names the settings it ignored.
- [ ] Introduce a typo in a key name. It appears in the same report, so a misspelling and a
      removed setting are told apart by reading, not by guessing.
- [ ] Saving a syntax error is refused, names the line, and the previous settings stay in
      effect.

### The agent

- [ ] Tray menu opens on right-click and offers Disable, Reload, Open configuration, Quit.
- [ ] **Disable, then press the hotkey with text selected: nothing converts.** Enable, press
      again: it converts. The icon changes visibly between the two states.
- [ ] The disabled tooltip says the hotkey does not convert — not that recording stopped.
- [ ] "Open configuration file" creates `%APPDATA%\LexiSwitch\config.toml` when absent and
      opens it.
- [ ] Editing and saving the file applies within ~2 s, with a confirming balloon.
- [ ] Changing the hotkey re-registers it; the old binding stops working, the new one works.
- [ ] Quit leaves no `lexiswitch.exe` running, and the hotkey is free for another application.

### Budgets and absence

- [ ] Idle for an hour: resident memory under 20 MB (`Get-Process lexiswitch`). Startup is
      measured at 13.1 MB.
- [ ] No outbound connection during an extended session (`resmon`, filtered to the process).
- [ ] The release binary contains none of `RegisterRawInputDevices`, `GetRawInputData`,
      `RIDEV_INPUTSINK`, `SetWindowsHookEx`, `CallNextHookEx` — and does contain `SendInput`
      and `RegisterHotKey`, which is what proves the search works.
- [ ] The release build launches and is not quarantined.

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

### No keyboard-observation capability survives in the release binary

**Observed 2026-08-07**, after `retire-keystroke-capture` removed the capture subsystem. This
is the evidence for the pivot's invariant that a release build contains no active general
keystroke-monitoring subsystem, and for the standard
[ADR-0011](adr/0011-no-keyboard-observation.md) inherits from ADR-0003: a reader auditing the
binary can confirm it rather than taking the source on trust.

Searched `target\release\lexiswitch.exe`:

```
must be ABSENT                          control: must be PRESENT
  RegisterRawInputDevices   absent        SendInput          present
  GetRawInputData           absent        SetClipboardData   present
  RIDEV_INPUTSINK           absent        RegisterHotKey     present
  SetWindowsHookEx          absent        OpenClipboard      present
  CallNextHookEx            absent
```

**The control column is the point.** A search that only ever reports "absent" cannot
distinguish a genuine removal from a search that was never working — the first attempt at this
check was in fact broken, reporting every symbol as present because `grep -c` exits non-zero
on zero matches. Symbols the program certainly does call are searched the same way, and their
presence is what makes the absences meaningful.

Binary is 0.54 MB against the 5 MB budget, down from 0.57 MB.

---

### The endpoint-protection experiment, after the removal

**Observed 2026-08-07**, on the release build with keystroke capture removed. It launches,
runs, and is not quarantined. Working set 13.8 MB.

Set against the pre-removal baseline recorded below, this is a null result, and it is worth
being precise about why rather than reporting it as a pass:

| | Runs? | Quarantined? |
|---|---|---|
| Before removal (2026-08-07) | yes | no |
| After removal (2026-08-07) | yes | no |

**Nothing was fixed, because nothing was broken at the time of measurement.** The pivot brief
§22 lists "the release build is not blocked because of typing-monitor behaviour" as a
completion criterion, and that criterion cannot be met by this work — not because the work
failed, but because there was no failing baseline for it to improve on. The 2026-08-05 block
was real and is recorded below; it simply does not reproduce, and whether the policy changed,
the rebuilt binary carries a different verdict, or the original detection was build-specific
cannot be separated from here.

What this *does* establish is a regression check: removing capture did not make the binary
look worse to endpoint protection. Keep running it; treat a future block as new information
rather than as the return of a known problem, since the attributes that plausibly caused the
original one — unsigned, zero prevalence, optimised, stripped, console-less, registering a
global hotkey, calling `SendInput` — all survive the pivot untouched.

---

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
