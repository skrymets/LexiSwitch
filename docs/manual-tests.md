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
.\target\release\lexiswitch.exe
```

### Setup

- [ ] Two or more keyboard layouts installed.
- [ ] Tray icon appears, and its tooltip reads `LexiSwitch`.
- [ ] **The icon is the ordinary application icon, not the red "no entry" one.** The program
      starts enabled, so the disabled icon at startup means the state and the icon disagree —
      which is exactly what happened on 2026-08-07 when the tray's boolean was renamed and a
      call site kept passing the old polarity. Checking only the tooltip did not catch it.
- [ ] Startup balloon names the selection hotkey. **If it does not appear, do not stop here:**
      right-click the tray and choose Disable, which fires a balloon on demand. If that one
      appears, only the startup balloon was missed; if it does not, the notification channel
      itself is not working and every failure the program reports is invisible.
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

### Tray clicks never reached the message loop

**Observed 2026-08-07.** The tray menu could not be opened by clicking the icon, through four
attempted fixes, each of which addressed a real defect and none of which was the cause.

The cause is that `GetMessageW` returns **posted** messages only. A message another thread
*sends* is never queued — it is dispatched straight into the window procedure, from inside the
`GetMessageW` call — and the shell **sends** notify-icon callbacks. This program does its work
in the message-loop handler, so every tray click reached the window procedure and stopped
there.

Measured with one variable, same window, same message:

```
SendMessage(hwnd, WM_APP+1, 1, WM_RBUTTONUP)   ->  nothing
PostMessage(hwnd, WM_APP+1, 1, WM_RBUTTONUP)   ->  handler runs, menu opens
```

The window procedure now re-posts sent application-range messages so the loop sees them. With
that in place the log shows what had been discarded all along, including `NIN_BALLOONSHOW` and
`NIN_BALLOONTIMEOUT` for the program's own startup balloon.

**What made this cost four attempts** is worth more than the fix. Each earlier attempt was
verified only on the half of the system reachable from inside the process — posting the message
by hand opened the menu, which proved the handling was correct and said nothing about delivery.
Two of the fixes were genuine bugs (a message-only window cannot take the foreground, which
`TrackPopupMenu` requires; and the documented message after `TrackPopupMenu` was missing) but
neither was the reported symptom, and both were presented as though they were.

The first `probe_tray` was useless here because it was *minimal*: it logged in the window
procedure, so it saw the callbacks and reported the shell was behaving. The difference between
a minimal probe and the real program is everything the program does, and the one difference
that mattered was hidden among the rest. A differential test is only as good as the differences
it reproduces.

---

### CLOSED: converting a selection in Notepad++ deletes it

**Reproduced 2026-08-07, diagnosed and fixed the same day — see "A held modifier turns the
copy into a deletion" below. The entry is kept as written, because the reasoning that failed
to find it is worth more than a tidy record.**

**Reproduced 2026-08-07, not diagnosed, not fixed.**

```
fresh tab                      -> empty
paste "Еуче шт Утпдшір"        -> present
select all, press Shift+Pause  -> DOCUMENT EMPTY
log: selection_requested from=de-DE to=en-US app=notepad++.exe
     outcome_failed: nothing is selected; nothing was changed
```

The program reports that it did nothing while the text disappears. On that path the only
synthetic input sent is `Ctrl+C`, and the code returns before any paste — and `Ctrl+C` does not
delete text in Notepad or Notepad++ when exercised in isolation
(`examples/probe_modifier.rs`, both with and without a modifier held).

**The control that has not been run** is the same sequence with LexiSwitch not running at all.
If the text still vanishes, the harness is at fault rather than the program, and several of the
dead ends above were self-inflicted in exactly that way.

Deliberately not pursued further. `convert_selection` replaces text the moment the hotkey is
pressed, and `add-candidate-chooser` removes that path entirely: the chooser reads the
selection, presents candidates, and modifies nothing until an explicit commit. Silently
mangling a selection is the failure mode the pivot exists to eliminate, so hardening the code
scheduled for deletion is the wrong place to spend. **Re-test this case once the chooser
lands** — if it survives the rewrite, it is a genuine defect in the read or replace primitives
rather than in the path being retired.

---

### What the shell actually sends a tray icon

**Observed 2026-08-07.** Three fixes to the tray menu in a row each addressed a real defect
and none of them made a right-click work. `examples/probe_tray.rs` settled it by registering
three icons at once and logging every callback the shell delivered to each.

| Icon | Window | Protocol | Tooltip | Right-click delivered |
|---|---|---|---|---|
| A | message-only | legacy | yes | 3 × `WM_RBUTTONUP` |
| B | top-level | legacy | yes | 2 × `WM_RBUTTONUP` |
| C | top-level | version 4 | **no** | 3 × `WM_CONTEXTMENU` |

758 callbacks in total. Two things follow, and both contradict what had been assumed.

**Right-clicks were never being lost.** The shell delivers `WM_RBUTTONUP` under either
protocol, *including to a message-only window*. The original menu failure was entirely
`SetForegroundWindow` failing on a message-only window, which `TrackPopupMenu` needs; the
message itself always arrived.

**Version 4 costs the tooltip.** Icon C received seven `NIN_POPUPOPEN` and seven
`NIN_POPUPCLOSE` events and displayed nothing. Under version 4 the shell stops drawing `szTip`
and asks the application to render its own tooltip. It buys nothing in exchange here, so it
was reverted.

The settled configuration is B: an ordinary top-level window that is never shown, on the
legacy protocol.

**A methodological note worth more than the fix.** Two rounds of this were spent chasing a
symptom that had already been repaired, because the app was being stopped with
`Stop-Process -Force`. That gives it no chance to run `Shell_NotifyIcon(NIM_DELETE)`, so the
shell keeps painting an icon whose owner is gone — and a ghost icon shows no tooltip and
swallows clicks, which is indistinguishable from the bug being hunted. Roughly ten of them
accumulated. Post `WM_COMMAND` with the Quit command instead; it runs the real shutdown path
and the icon removes itself.

The tooltip is the cheap discriminator throughout: it needs no message loop, so an icon with
no tooltip is mis-registered or dead, never mis-handled.

---

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

---

## Resident memory and network, an hour idle (tasks 6.3 and 6.4)

**Measured 2026-08-07** against `target\release\lexiswitch.exe` at commit `9b22ea2`, sampled
every five minutes for an hour. The later chooser-activation fix (`a98911c`) touches only the
window's handling of `WM_ACTIVATE` and nothing on the idle path.

| | working set | private working set | handles |
|---|---|---|---|
| idle, before any conversion (15 min) | 13.6 → 13.7 MB | 1.8 → 1.7 MB | 182 |
| peak, just after the first conversion | 22.2 MB | 2.9 MB | 300 → 286 |
| end of the hour (45 min later) | 21.1 MB | 1.7 MB | 287 |

**No leak.** Across the 45 minutes following a conversion every figure drifts *down* — working
set 22.2 → 21.1 MB, private 2.9 → 1.7 MB, back to where it started before any conversion ran —
and the handle count does not move off 287. Nothing accumulates per conversion.

**The 20 MB budget is exceeded on working set, and met with two decimal orders of margin on
private bytes.** The step at the first conversion is UI Automation and the shell being mapped
in — `uiautomationcore.dll` (4.1 MB), `shell32.dll` (7.6 MB) and `windows.storage.dll`
(8.6 MB) account for most of it. Those are shared pages, already resident for every process on
the machine, so they cost the system nothing extra; they are counted against this process
because working set counts what is mapped, not what is paid for.

So the honest answer depends on a question the budget does not currently ask: **which measure
does "resident memory under 20 MB" mean?** On `WorkingSet64`, the number a user reads in Task
Manager, this fails at ~22 MB. On private working set it passes at 2.5 MB. Both are recorded
rather than picking the flattering one. The budget should be restated to name its measure
before this task is called passed or failed — it cannot be settled by measuring harder.

**No outbound connection.** Every sample reported `net=none`: no TCP connection to any
non-loopback address, and no UDP endpoint, across the whole hour including the conversions.
This is the runtime observation task 6.4 asks for, and it agrees with the import-table evidence
already recorded above — the binary links no networking library at all.

---

## Layout handles are not 32-bit (2026-08-07)

`Ctrl+Enter` converted the text but never changed the input language, and only for Ukrainian.
Measured with `cargo run -p lexiswitch-win --example probe_langswitch -- <layout>`, which asks
for a named layout and then reads back what the foreground window's language actually is:

| layout | HKL as reported | switch honoured |
|---|---|---|
| `en-US` | `0x00000000_04090409` | yes, within 100 ms |
| `de-DE` | `0x00000000_04070407` | yes, within 100 ms |
| `uk-UA` | `0xFFFFFFFF_F0A80422` | **no, ignored entirely** |

A layout that is not the default for its language gets a handle whose high word is a device
identifier rather than a language id, and on 64-bit those come back **sign-extended**. The code
stored `HKL` as a `u32`, so Ukrainian's handle was truncated to `0xF0A80422` and widened again
to `0x00000000_F0A80422` — a handle naming no installed layout. `WM_INPUTLANGCHANGEREQUEST`
then failed in the one way that looks identical to an application declining it: silently, with
no error at any layer.

The two layouts whose handles happen to have bit 31 clear survived the round trip and worked,
which is what made this present as "layout switching does not work" rather than "one layout
does not work". A probe that tested only the convenient handle would have reported success.

**Do not narrow an `HKL` to 32 bits.** It is a handle, and handles are pointer-width.

---

## A held modifier turns the copy into a deletion (2026-08-07)

Selecting text in Notepad++ and pressing `Shift+Pause` deleted it. No chooser appeared, and the
log said `Selection could not be read: nothing is selected; nothing was changed` — which was
true of every code path this program runs, and yet the user's text was gone.

It was destroyed by the one keystroke the read path sends. `RegisterHotKey` consumes the
`Pause` of `Shift+Pause` but nothing releases the `Shift`, and the user's hand is still on it
milliseconds later when `Ctrl+C` goes out. The target receives `Ctrl+Shift+C`.

Isolated to that single keystroke, same selection, same document:

| sent to Notepad++ | copied | document |
|---|---|---|
| `Ctrl+C` | the selection | intact |
| `Ctrl+C` with `Shift` held | nothing | **emptied** |

Notepad tolerates the same chord, which is exactly why this took three sittings to find: every
reproduction attempt in Notepad passed, and Notepad++ was reached only through the real hotkey,
never through a probe.

**`probe_modifier` was written to catch precisely this** — its own module comment says a held
`Shift` "would explain both halves of a report that otherwise makes no sense: the copy produces
nothing *and* the selection disappears." It was run against Notepad, came back clean, and the
hypothesis was dropped. The probe was right and the target was wrong. A probe that exercises
one application is evidence about that application and nothing else.

**Fixed** by releasing any held `Shift`, `Alt` or `Win` before synthesising any chord
(`inject::release_held_modifiers`). `Ctrl` is left alone: the chords supply it themselves and a
user holding it changes nothing about what they mean. The release is not undone — the physical
key is still down, so the user's own release follows and a duplicate key-up is harmless, while
re-pressing on their behalf could leave a modifier stuck down.

Verified after the fix, in Notepad++ through the real hotkey: chooser opens, `Escape` leaves the
document intact, `Enter` converts correctly, the prior clipboard comes back, and one `Ctrl+Z`
restores the original.

---

## The paste is asynchronous, and restoring the clipboard too early loses it (2026-08-07)

Conversion did nothing useful in Chrome. The matrix showed why in one cell: the field ended up
holding `PRIOR-CLIPBOARD` — the harness's *previous* clipboard contents — instead of the
conversion.

`SendInput` returns once a keystroke is enqueued, not once it is applied. The replacement put
the converted text on the clipboard, sent `Ctrl+V`, waited a fixed 180 ms and then restored the
previous contents. A native edit control reads the clipboard well inside that window; Chrome
does not. It read *after* the restore and pasted what had just been put back.

The failure is silent and total: the paste succeeds, the program reports success, and the user's
document receives whatever they happened to have copied earlier.

**Fixed** by waiting for the target to actually take the clipboard rather than assuming a delay
is enough (`clipboard::wait_for_consumer`). Opening the clipboard is the one observable part of
reading it, so the wait watches for another process to open it and then let go, capped at two
seconds. Nothing waits the full two seconds in practice — the wait ends when the consumer
finishes — so an application that never reads the clipboard at all pays the timeout and nothing
else does.

## The compatibility matrix

`docs/testing/run-matrix.ps1` runs four scenarios — apply, apply-and-switch, cancel, and a
selection no layout converts — against Notepad, Notepad++, Chrome and Edge, and reports expected
against actual for both the resulting text and the resulting keyboard layout. Latest results are
in `docs/testing/matrix-results.md`.

It exists because every defect in this project so far was found in whichever application
happened to expose it, and fixed against that one application: a held modifier that destroys a
selection in Notepad++ was probed in Notepad and declared clean, and a clipboard race that
breaks Chrome could not be seen in either editor. Sixteen cells run in a few minutes and answer
the question the individual probes could not — what is the state of the whole thing right now.

**A caution on reading it.** Two of its early failures were defects in the harness, not the
product: a hardcoded Ukrainian layout handle with the wrong constant, and a window search that
matched an Edge window when Chrome was asked for. A third was the harness checking the clipboard
sooner than the product's own worst-case restore. A red cell is a question, not a verdict.

---

## What "the replacement failed" can and cannot be detected from (2026-08-07)

A read-only field in the compatibility matrix caught the system reporting `outcome_applied` for
a conversion that changed nothing. `SendInput` reports keystrokes *enqueued*, never applied
(ADR-0010), so nothing in the paste path knows whether the target acted on it.

The clipboard offers a partial answer. Pasting requires reading the clipboard, so a target that
never opened it cannot have pasted from it. That signal is sound here — measured with
`probe_clipboard_consumer`, which writes the clipboard and sends no paste at all:

| clipboard written | consumer seen within 2 s |
|---|---|
| plain text | no |
| with the history-exclusion formats set | no |

So nothing on this machine reads the clipboard opportunistically, and a consumer observed after
a paste chord really is the target. Clipboard History, cloud clipboard and third-party managers
would each have made this useless; none of them fired.

**It is still not sufficient.** Chrome opens the clipboard while processing `Ctrl+V` and *then*
discovers the field is read-only, so the consumer appears and the document does not change. The
signal rules out "the target never processed the paste"; it cannot rule out "the target
processed it and declined".

**Implemented**: a replacement whose clipboard was never taken is reported as
"Conversion was not applied" rather than as success, and no layout switch follows it.

**Not implemented**: detecting a paste that was read and refused. That needs a verification read
of the target after the commit — feasible where UI Automation exposes the focused element's
text, unavailable elsewhere, and carrying its own risk of reporting a false failure when an
application legitimately transforms what was pasted. Recorded rather than attempted, because a
false "not applied" on a conversion that worked is worse than the case it would fix.

## Notepad++ converts the current line when nothing is selected

Noticed in the matrix, not a failure. Every other target reports "nothing is selected" when the
hotkey is pressed with a collapsed caret; Notepad++ opens the chooser instead, because its
`Ctrl+C` copies the current line when there is no selection. Committing there would convert the
whole line the caret is on.

Left as it is. The behaviour belongs to the editor's copy command, the chooser shows exactly
what would be replaced before anything happens, and suppressing it would mean second-guessing
what the user's own editor decided a copy means.

---

## The paste settle must come before the clipboard wait (2026-08-07)

The replacement puts text on the clipboard, sends `Ctrl+V`, waits for the target to take it, and
then restores what was there before. The order of the settle and the wait was got wrong in both
directions before it was got right, and each mistake produced a different failure:

| order | what breaks |
|---|---|
| settle, then wait | a target reading inside the settle is never observed — used as evidence, this reported a *working* Notepad conversion as "not applied" |
| wait, then settle | Chrome watches the clipboard and opens it the instant anything is written, so the wait returns on that observation, the restore runs, and the paste reads the restored value — the field ends up holding the user's previous clipboard |

Both were measured, the second twice: it was introduced to fix the first, and put `PRIOR-CLIPBOARD`
into the Chrome row of the matrix within one run.

**Settle first.** It skips past the change-observation so the wait sees the paste's own read. The
cost is the first row of the table, which is why the "was it consumed" flag is reported for
diagnostics and never used to decide anything.

The wider lesson is in the second row: a clipboard *open* is not a paste. Chrome opens the
clipboard when it changes, when it pastes, and when it declines to paste into a read-only field.
Nothing about opening it distinguishes those.

---

## The chooser's activation bug, reproduced in the recorder (2026-08-07)

The shortcut recorder cancelled itself the instant it opened, logging
`shortcut_recording_cancelled` with nothing visible on screen. Same cause as the chooser's
vanishing window a few hours earlier: taking the foreground from another process emits
activate, deactivate, activate before the user has touched anything, and a window that treats
the middle one as "clicked away" ends its own transaction before its loop starts.

Worth recording not as a new finding but as the same finding twice. The fix had been written,
understood and commented — and it lived in the chooser's own state struct, so the second window
did not get it. Extracting `winui` moved the *foreground handling* into shared code but left the
*arming rule* behind in each window, which is precisely the part that was subtle.

Both windows now arm only after the foreground has settled, and both confirm against
`GetForegroundWindow` rather than trusting the message.

---

## Shell and shortcut verification (2026-08-07)

Against the release build unless noted. The agent's own log is quoted because it is the only
channel that reports outcomes reliably — balloons have been observed not displaying at all on
this machine.

| what | result |
|---|---|
| Startup with the shortcut held by another application | Keeps running. `hotkey_conflict: … Hot key is already registered.` Tray icon present and receiving shell messages. Previously exited with code 1 and no icon. |
| Convert from the tray while the shortcut is held | Works. `selection_requested` then `outcome_applied`; document converted. |
| Record a replacement shortcut | Works. `shortcut_changed`, written to the configuration file, `config_reloaded`, then converted with the new combination. No restart. |
| Conflict clears when the shortcut is released | Works. Holder stopped, tray touched, `hotkey_conflict_cleared`, then converted on the recovered shortcut. |
| Configuration reload applying a valid change | Works, as part of the recording flow above. |
| Release build, not only debug | The compatibility matrix runs against the release binary. |

**Not executed, and why.** Starting with one keyboard layout installed, and becoming usable when
a second appears, would both mean changing the user's Windows keyboard configuration. The logic
is unit-tested and the recovery path is the same re-check that clears a shortcut conflict, which
is verified; what is untested is the real startup and the real transition.

**Not yet executed**: reload rejecting an invalid file end to end (covered at unit level), and a
rebind that fails after a previous one succeeded.

## The layout switch is advisory, and occasionally declined

Chrome's apply-and-switch row failed once in a full matrix run — the text converted and the
layout stayed `en-US` — then passed three times out of three when run on its own.

Consistent with what the request is. `WM_INPUTLANGCHANGEREQUEST` is posted, an application may
ignore it, and there is no return value that says it did; the outcome deliberately says
"requested" rather than "switched". Recorded rather than chased: an intermittent decline is the
documented behaviour of the mechanism, not a defect in the caller.

## Two harness faults that read as product failures

A matrix run reported 19 of 26, with all five Notepad rows saying "target unavailable" — while
Notepad was running with a window the whole time. The harness could not take the foreground and
reported that with the same message it uses for a missing window, so the cause was invisible.

Both are fixed: the two conditions now report differently, and the focus routine minimises a
window that will not yield after a few polite attempts, which is the only thing observed to
move one. The same run's remaining failures were the cascade from typing into nothing.

Worth keeping in mind when reading a red cell: this harness has now produced more false
failures than true ones.

---

## Manual checklist results (2026-08-07)

Executed by the user against the release build. Full working copy in
`docs/testing/manual-checklist.md`.

**The two layout conditions, which had never been run**, because they mean changing the
machine's installed keyboard layouts:

- **Starting with one layout installed** — starts, does not exit. Balloon: *"Conversion needs at
  least two keyboard layouts, and 1 is installed. Add a second layout in Windows settings;
  LexiSwitch will pick it up without being restarted."* Tray icon a yellow warning triangle,
  tooltip *"1 keyboard layout is installed, two are needed"*. `Shift+Pause` does nothing.
- **Adding a second layout while it runs** — noticed without a restart. Icon returns to normal,
  balloon names the shortcut, and conversion works. This was the half of task 5.1 that had only
  ever been reasoned about.

**The five states are visibly distinct.** Enabled, disabled and shortcut-conflict all confirmed
by eye as different from one another, with the conflict naming the unavailable combination. Too
few layouts is the fourth, above. The runtime-error state was not reached, and there is still no
way to reach it deliberately.

**Diagnostics** reports the state, layouts, configuration path and logging setting, with no
selected text or conversion results in it.

**Cancelling by clicking away** leaves the document and the clipboard untouched.

**Microsoft Word** converts correctly — an application outside the matrix and outside the two
engines it covers.

### OPEN: IntelliJ IDEA reads the selection only sometimes

**Reported 2026-08-07. Not diagnosed, not fixed. Left open deliberately.**

In an IDEA XML editor, selecting `БщкпфтшяфешщтЮ` (`<organization>` typed under the Ukrainian
layout) and pressing `Shift+Pause` gives *"Selection could not be read: nothing is selected;
nothing was changed"*. Moving the caret, reselecting the same text and pressing again produces
the chooser and converts correctly. Repeating alternates between the two outcomes with no
pattern the user could see.

Both halves are consistent with the read failing rather than the conversion: nothing is
modified in the failing case, and the message is the honest one.

Worth noting what it is **not**. The Notepad++ deletion had the same shape — a copy that
produced nothing — and turned out to be a held modifier turning `Ctrl+C` into `Ctrl+Shift+C`.
That fix is in and Notepad++ is clean, so if a modifier is involved here it is a different one,
or IDEA is doing something with focus between the hotkey and the copy. IDEA is a JetBrains IDE
and this file already records that IDEA's accessibility support is partial (`## Password-field
detection coverage`), so the automation path may be intermittently available and the clipboard
fallback then racing something.

Not investigated further at the user's direction.

---

## One instance per session (2026-08-07)

Verified with `docs/testing/shell-checks.ps1`, 14 of 14:

- A second launch declines, exits `0`, and says *"already running; the running one has been asked
  to show itself"*. The first is untouched and exactly one process remains.
- The running instance logs `second_instance_declined` and shows a balloon, so a user who has
  lost track of it is pointed at it rather than left with a program that appeared to do nothing.
- Two started at the same moment leave exactly one. The decision is a named mutex, made in the
  kernel; a window search would have let both pass.
- A start after a **hard kill** succeeds — nothing is left behind, which a lock file could not
  promise.
- A genuine conflict with another application still reports and still recovers. That is now the
  only way to reach the conflict state, which is what it always described.

## The shell can hold the foreground, and no amount of asking moves it

Chrome rows of the matrix began failing with "could not focus". The foreground window turned out
to be `Shell_SecondaryTrayWnd` — the taskbar on the second monitor, owned by Explorer. While the
shell holds the foreground, `SetForegroundWindow` from another process returns false every time.
Retrying does not help, and minimising a taskbar does nothing.

A tap of `Alt` releases the lock. But **only tap it when the shell is the blocker**: `Alt`
activates a menu bar, and in a browser that moves focus off the page — tapping it
unconditionally fixed the focus and broke the field the test types into, turning "could not
focus" into "seeding failed". The harness now checks the blocking window's class first.

Worth remembering for the product, not only the harness: `winui::take_foreground` could meet the
same condition. It has not been observed there, because the window it takes the foreground from
is the application the user just selected text in — never the taskbar. Recorded rather than
pre-emptively worked around.

## Seeding is setup, and setup gets retried

The same investigation produced three "seeding failed" cells that meant only that Chrome had not
finished becoming interactive on a machine with 27 GB of 31.6 GB in use. Seeding is not the thing
under test, so it now retries three times before failing the case. Chrome went from 1 pass in 4
to 4 in 4, and the full matrix to 24 of 24.

This is the third harness fault in a row that presented as a product failure. The count is worth
keeping honest: a red cell in this matrix is a question, and more often than not the answer has
been about the harness or the machine rather than the program.

---

## A globally held combination cannot be typed into the recorder (2026-08-07)

Trying to verify what happens when a recorded replacement *also* fails to register, by having
another application hold `Ctrl+Alt+K` and then recording `Ctrl+Alt+K`, produced nothing: the
recorder captured no combination at all and the log showed only that the recording was
cancelled.

The reason is how global hotkeys work. `RegisterHotKey` means the combination is delivered to
the registering application and to nobody else, so pressing it while the recorder has focus
sends no key event to the recorder. There is nothing to capture.

Two consequences, and the first is a benefit.

**An unavailable combination cannot usually be recorded in the first place.** The user presses
it, nothing appears, and they try another. The failure they would otherwise have hit — record
it, watch it be refused, try again — mostly cannot arise.

**But the silence is unexplained.** From the user's side, a key combination that another
application holds simply does nothing in the recorder, with no indication why. That is a real
rough edge. It is not a defect in the recording path; it is the operating system doing exactly
what the shortcut was registered for. Left as it is for now, and recorded so it is not
rediscovered as a mystery.

**The failure-to-rebind path is still reachable in principle** — a combination can be free when
recorded and taken by the time registration is attempted — and the code handles it: the previous
shortcut is re-registered and the recorder reopens with the reason. Arranging that race
deliberately is beyond this harness, so that branch is covered by reading rather than by test,
and this is the note saying so.

---

## Hot reload never happened on an idle agent (2026-08-07)

Testing that an invalid configuration keeps the last known good produced nothing at all: the
agent's log showed the file being read once at startup and never again. Not a rejection, not a
reload — no attempt.

The configuration poll runs in the message-loop handler, and the handler only executes when a
message arrives. `GetMessageW` blocks until one does. An agent nobody is touching receives none,
so an edit to the configuration file was noticed only when something unrelated happened to wake
the loop — a tray hover, a hotkey press, anything. On a machine the user was not interacting
with, the documented "applies within about two seconds" did not happen at any point.

**Invisible until the pivot removed Raw Input.** While keystrokes were observed, every key
pressed anywhere delivered `WM_INPUT` and pumped the loop continuously, so the poll ran
constantly by accident. Removing observation removed the heartbeat with it — exactly as it
removed outcome reporting, which needed `Waker` for the same reason and was found the same way.
This is the second thing that turned out to be running on borrowed messages.

**Fixed** with a two-second `SetTimer` on the message window. The timer does nothing except
arrive; waking the loop is the whole job, and the handler does the work as before.

Found only because a test asserted on the agent's own log rather than on the outcome. Asserting
"the previous configuration survives" would have passed — it did survive, because nothing ever
tried to replace it.

## Case conversion: what was verified, and three harness faults on the way (2026-08-08)

The case transaction (ADR-0014) landed with 200 core tests, 20 new shell checks, and a clean
matrix. What is worth recording is not the passes but what the failures turned out to be.

**Win+Ctrl+Pause registers.** The default binding for the second shortcut was the plan's
riskiest assumption: Windows reserves Win-key combinations, and `Win+Pause` itself opens System
properties. `probe_hold_hotkey` takes it without complaint, so the default ships working rather
than shipping a conflict for the user to resolve.

**Binary 0.63 MB → 0.74 MB.** The whole feature — two core modules, a third window — costs
0.11 MB against a 5 MB budget.

### The debug binary was stale, again

The first Case run failed every window check, and the agent's log showed the command never
arriving. The cause was a debug binary eleven minutes older than the release one: the last
`cargo build -p lexiswitch` ran *before* the wiring edit, and `cargo clippy` produces no binary
at all. This is the third time a stale build has presented as a broken feature in this project.
`cargo test --workspace` does not help either — it builds test binaries, not the agent.

### Three groups depended on a configuration nobody set

Conflict, Recovery and Gating all hold or press `Shift+Pause`, and none of them said so. They
passed for as long as the live configuration happened to hold the default. The moment it did
not — the operator had rebound it to `Shift+Win+Pause`, and later a debugging run of mine
persisted `Ctrl+Alt+K` — three checks failed for reasons that had nothing to do with the code.

Both `shell-checks.ps1` and `run-matrix.ps1` now write a known configuration at the start and
restore the operator's at the end. A red cell should mean the product.

### Existing is not focused

After the foreground handshake moved into `winui`, the recorder group failed to register a
replacement while the recorder itself demonstrably opened. `Activation::present` retries
`SetForegroundWindow` up to nine times, so a window is findable by `EnumWindows` for up to half
a second before it can receive a key — and the harness typed as soon as it found one. The
keystrokes went to whatever still had focus, which is indistinguishable from the feature being
broken. Both window waits now wait for the foreground, not for existence.

Reproducing the same steps by hand with a three-second settle worked first time, which is what
identified it: the product was never wrong.

### A case-insensitive test suite about case

The first green Case run reported `got 'A B'` for a check written against `'a b'` — and passed.
PowerShell's `-eq` is case-insensitive, so three checks in a suite whose entire subject is
letter case could not fail. Now `-ceq`. The same run also showed the pair check had navigated to
the wrong style, because it counted rows from the top of a list that grows as styles are
promoted; it now discards the counts first so `Home` means the same thing every time.

### Recorded, not fixed

- **One intermittent.** "the document is untouched" failed once in five runs of the Case group
  and has not recurred. No behaviour was changed between the failure and the passes — only
  diagnostics were added — so it is an open intermittency in the harness, not something solved.
- **Rendering is unverified by machine.** The group boundaries, the dimming of rows and toggles,
  the expanded view and the footer are visual claims. They are in `docs/testing/manual-checklist.md`
  for eyes rather than asserted here.

## Both default shortcuts become Win+<modifier>+Pause (2026-08-08)

`convert_selection` moves from `Shift+Pause` to `Win+Shift+Pause`; `change_case` stays at
`Win+Ctrl+Pause`. The pair is now one gesture with one finger moved rather than two unrelated
combinations, and both sit clear of application bindings.

`Win+Shift+Pause` was proven registrable with `probe_hold_hotkey` before anything was changed,
the same way `Win+Ctrl+Pause` was. Windows reserves Win-key combinations in principle and
`Win+Pause` opens System properties, so neither default can be assumed.

**The interesting consequence is that the hotkey now holds Win.** `release_held_modifiers`
already covered `VK_LWIN`/`VK_RWIN`, so nothing needed changing — but the harnesses did. Both
now press the full combination and keep *both* modifiers down past the Pause release, which is
the condition that function exists for. A leak would deliver `Win+Ctrl+C` instead of `Ctrl+C`,
and `Win+C` is a shell binding.

That makes the matrix a stronger test than it was: it previously held only Shift.

Verified with both defaults in force: **47/47 shell checks**, **24/24 matrix**, 175 unit tests,
clippy and the release build clean. Notepad++ passes, which is the application where a held
modifier once emptied the document rather than copying from it.

**A note on the operator's own configuration.** It already read `Shift+Win+Pause`, chosen by
hand some time ago — the same combination as the new default, written in the other order.
`same_binding` compares component by component precisely so that these are recognised as one
thing; nothing had to change for that file to keep working.

**Not re-run:** `docs/testing/matrix-results.md` now holds this run, replacing one that
predated the case work and carried a stale Chrome failure.

## A held modifier defeats the copy, and releasing it once is not enough (2026-08-08)

Reported: with `change_case` rebound to `Alt+Win+Pause`, case conversion fails in Notepad++ when
the keys are pressed Win-then-Alt, and works Alt-then-Win. Order never matters in Notepad, and
`Shift+Win+Pause` works either way.

### What the measurements said

**Any held modifier stops the copy.** In Notepad++, against the same selection, four trials each:

| sent | result |
|---|---|
| `Ctrl+C` | copied, 4/4 |
| `Ctrl+C` with `Alt` held | **nothing, 4/4** |
| `Ctrl+C` with `Win` held | **nothing, 4/4** |
| `Ctrl+C` with `Shift` held | **nothing, 4/4** |

So this is not about `Alt` specifically, and `release_held_modifiers` is entirely load-bearing.

**Notepad is immune because it is never asked.** It exposes a UI Automation text pattern, so the
selection is read without sending `Ctrl+C` at all. No chord, no modifier problem. That is the
whole Notepad/Notepad++ asymmetry, and it is structural rather than a difference in tolerance.

**Releasing once is undone by the keyboard.** A held key repeats — roughly thirty times a second
— so a synthetic release survives about 33 ms before the hardware presses it again. The old code
released once, slept 30 ms, and sent the chord: one repeat period. Simulating auto-repeat
reproduces the failure reliably, which nothing else had:

| held with simulated repeat | before | after |
|---|---|---|
| 300 ms | 3/4 opened | 4/5 opened |
| 1400 ms | **1/4 opened** | 3/5 opened |

### What was wrong in my own first diagnosis

I measured Notepad++'s copy latency at 32 ms and called the 60 ms budget a thin margin. That
figure was inflated by the measurement itself: polling with `Get-Clipboard` *opens* the
clipboard, contending with the application being measured. Polling the sequence number instead —
no handle, no contention — puts it at about 10 ms. The 60 ms budget had roughly six times the
margin, not two. The fixed-timeout race was real but it was not this bug.

### Fixed

- The read waits for the clipboard sequence number to change rather than sleeping a flat 60 ms,
  with a 600 ms deadline. Fast targets return sooner than before; slow ones stop being reported
  as an empty selection.
- `release_held_modifiers` now re-releases and waits for every modifier to read as up for 90 ms
  — about three repeat periods — before the chord is sent, up to 2.5 seconds.
- The last releases and the chord go out in **one** `SendInput` batch, so nothing injected can
  interleave between them.
- Failing to settle is reported as `ModifiersHeld` — "a key from the shortcut is still held
  down" — instead of "nothing is selected". The first is a statement about the keyboard and is
  true; the second is a statement about the document and was false.

### Not fixed, and why

**A modifier that is still physically down cannot be worked around.** `SendInput` is atomic
against other injected input but not against the hardware, so a repeat event can still land
between our release and our chord. Roughly one trial in five still fails while a key is held
down continuously through the transaction. The remedy is the one the code now takes: wait for
the user to let go, and say so plainly if they do not.

Verified no regression: 47/47 shell checks, 24/24 matrix, 175 unit tests.

## Manual group D, and the forced joins it found (2026-08-08)

Ten rendering items, run by hand against the release build. Seven confirmed as written. The
other three, and one bug the checklist did not think to ask about.

### Identifier styles were fusing every line into one name

Reported from a four-line selection:

```
cs key sent
cs key received
cs key blocked
cs key notified
```

Every identifier style — `snake_case`, `camelCase`, `dot.case`, all eight — returned a single
run with no line breaks at all: `cs_key_sent_cs_key_received_cs_key_blocked_cs_key_notified`.
The prose styles preserved the lines correctly. **Both line options were unset.**

This was my design, and it was wrong. Identifier styles discarded every separator they found on
the grounds that the style supplies its own delimiter — true for a space, for `_`, `-` and `.`,
and false for a line break, which does not separate *words* but *identifiers*. Discarding it
made `Lines: Keep` a setting that kept nothing, and made the fused result unavoidable rather
than a choice.

**Fixed.** A multi-line selection is a list of lines, each one its own identifier. The break
survives exactly as written, so a Windows file keeps its `\r\n`. Whitespace around each line is
still the style's to discard. Joining or removing the breaks is still available and is now the
only way to get the fused result. Word position resets per line, so `camelCase` lowercases the
head of *every* line rather than only the first — without that, `One two\nThree four` came back
as `oneTwo\nThreeFour`.

The line options consequently stop reporting "no effect" on multi-line text, which the dim rule
gets for free: it and the behaviour are the same predicate.

### D7 — the window blinked while scrolling

`InvalidateRect` was asking Windows to erase the background first. Every paint here fills the
whole client area before drawing, so the erase contributed one frame of blank window and
nothing else — invisible at one repaint, a flicker at scrolling speed. Now `false` in all three
windows, the chooser and the recorder included, which had it too.

### D9 — the list scrolled with nothing to say so

At 150% the window is clamped to the work area and the list scrolls, but the only evidence was
that pressing Down eventually revealed more. A proportional indicator is now drawn in the right
margin when the list does not fit.

### D5 — not a bug

Toggles 3 and 4 are dim for `snake_case` and still respond to the keys. That is correct: dim
means "would not change *this* style's result", not "disabled". The options are global, so a
user who happens to have an identifier style selected must still be able to set one — and the
box showing `[x]` is the truth about the setting.

Verified after the fixes: 182 unit tests, 47/47 shell checks, 24/24 matrix.

## Copying a result without applying it (2026-08-08)

`Ctrl+C` / `Ctrl+Ins` in either transaction window puts the selected result on the clipboard,
leaves the document alone, and keeps the window open. `H` decides whether it may enter Windows
Clipboard History — off every time the window opens.

### The interaction that made this more than a keystroke

The transaction owns the clipboard and restores it when it ends: `cancel` immediately,
`commit` after the paste. Both would have overwritten the copy — one instantly, one a moment
later. So `Transaction::clipboard_before` stopped meaning *what was there* and became *what to
restore*, renamed accordingly, and a deliberate copy replaces it.

**A second trap inside the first.** The restore wrote with plain `set_text`, so a copy the user
had asked to keep out of Clipboard History would have been put *into* history at the end of the
transaction — undoing the toggle at the one moment nobody is looking. The restore now carries the
exposure flag with the text, as one value rather than two.

### The check that could not fail

The first green Copy run asserted "the document and the copy are different styles" after
committing — and that check could not fail, because reading the document back *is* a copy, so
the clipboard had already been overwritten before it was examined. Reordered to read the
clipboard first. It now reads `clipboard 'HELLO WORLD'`, `document 'hello world'`, which is the
whole point of the change actually being asserted.

The seed was mixed-case for the same reason: with `hello world`, applying `lowercase` produces
the seed, and "the document holds the applied style" passes whether or not anything happened.

### The stale binary, for the fourth time

A `cargo build --release` failed with "Access is denied" because the agent was holding its own
executable. The failure scrolled past, the old binary started, and a full matrix run went green
about code from an hour earlier.

Both harnesses now refuse to run when a binary under test is older than the source, naming
which file. Examples are excluded from what the *agent* is compared against — editing a probe
cannot change the agent, and a guard that cries wolf is one somebody switches off.

### Notepad++ minimised and still foreground

Two matrix runs reported five Notepad rows as "could not focus". Notepad++ was **minimised and
still the foreground window** — a state in which Windows refuses `SetForegroundWindow` from any
other process, and which the harness's Alt-tap does not clear. Restoring it, tapping Alt, then
minimising it again releases the lock. Left in the operator's hands rather than automated: it is
a condition created by running probes, not by the product.

Verified: 182 unit tests, **57/57 shell checks** (10 new), 24/24 matrix, release clean at
0.75 MB.

## Drawing an empty string killed the agent (2026-08-08)

Reported: selecting a couple of spaces and opening the case window left it half drawn,
unresponsive, and then the process was gone.

### Exit code 0xC000041D

`STATUS_FATAL_USER_CALLBACK_EXCEPTION` — an exception inside a Win32 callback, which Windows
answers by terminating the process. Not a Rust panic: no message on stderr, nothing in the
event log, and a `catch_unwind` added around the whole window procedure never fired. That last
result is what ruled panics out rather than leaving it a guess.

Instrumenting the paint located it exactly:

```
DIAG  f: room=72
DIAG  g: preview=""      <- last line before the process died
```

**`DrawTextW` was being handed an empty slice.** windows-rs passes the empty `Vec<u16>`'s
dangling pointer with a count of zero, and the fault happens inside the window procedure.

Whitespace is simply the easiest way to reach it: every identifier style renders `"   "` to an
empty string, and that preview was the first empty text any of these windows had ever been asked
to draw. Any empty string anywhere in any of the three windows would have done the same.

**Fixed** in `winui::Pen::write` — nothing to draw returns early — and in both expanded views,
which call `DrawTextW` directly rather than through it. The `catch_unwind` guard was kept even
though it did not catch this one: a genuine panic in a paint would otherwise still kill the
agent, and finding that out the same way twice is not a good use of an afternoon.

### The stale binary, and a guard for it

A `cargo build --release` failed with "Access is denied" — the agent was holding its own
executable. The failure scrolled past, the old binary started, and a full matrix went green
about code an hour old.

Both harnesses now refuse to run when a binary under test is older than the source, naming the
file. Examples are excluded from what the *agent* is compared against, after the guard's first
firing was a false positive on a probe edit — a guard that cries wolf is one somebody switches
off.

### Notepad++ minimised and still foreground

Five Notepad rows failed two runs with "could not focus". Notepad++ was **minimised and still
the foreground window**, a state in which Windows refuses `SetForegroundWindow` from any other
process and which the harness's Alt-tap does not clear. Restore it, tap Alt, minimise it again.
Left to the operator: it is a condition created by running probes, not by the product.

### Labelling and layout, from the same pass

`H allows clipboard history` and `H keeps it in clipboard history` were two readings of the same
words, and neither said which way the option was currently set. Both windows now draw a real
checkbox — `DrawFrameControl`, not a `[x]` in text and not a `☑` the font may not have — with
one unambiguous label: **keep the copy in clipboard history**.

The strip options use the same checkbox, the footer's bands are fenced with rules, and the items
inside a band are separated with `│` rather than run together into something that reads as a
sentence. Everything that ends the transaction now sits on one line.

Verified: 183 unit tests, **61/61 shell checks** (4 of them new regression checks for the empty
string), 24/24 matrix.

---

## The copy keys get a footer line of their own (2026-08-08)

The history checkbox went off the right edge of both windows the moment the footer strings grew
`<` and `>` around the key names. Two faults, not one.

Its position was computed from `left.chars().count() * pen.char_width`, and `char_width` is
`tmAveCharWidth` — an *average* over a proportional font. Estimating a string's width from it is
wrong by however much the string differs from the font's average, and the error is unbounded.
`winui::Pen::measure` (`DT_CALCRECT`) now measures, and the checkbox group is anchored to the
right edge with the prose taking whatever room is left and ellipsising into it.

That alone was not enough. The line it shared grew with content — the chooser names the target
layout in it, and the case window swaps `<Enter> - apply` for `<Enter> - closes, changing
nothing` — so the control's room depended on the selected row. Copying now has a line to itself,
which is fixed-width, and the checkbox cannot be crowded off it by anything.

Adding that line meant four edits, not one: the paint, the height function (`height_for` /
`wanted_height`), and the separate footer constant the expanded view subtracts to find where the
scroll indicator goes. Nothing catches a mismatch between them but the eye.

Verified: clippy clean on `--all-targets`, 183 unit tests, 34/34 in the Case and Copy groups.
