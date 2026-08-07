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
