# Spike findings: selection replacement delivery

**Date:** 2026-08-06
**Change:** `openspec/changes/add-manual-layout-conversion` (task 8.6)
**Prompted by:** a report that `Shift+Pause` returned mangled text in modern Notepad

How a converted selection is *delivered* to the target application underpins
[ADR-0010](../adr/0010-paste-selection-replacement.md). This document records what was
*observed*, not what was decided — an observation stays true, a decision can be superseded.

## Answers

| Question                                                    | Answer                                                                                  |
|-------------------------------------------------------------|-----------------------------------------------------------------------------------------|
| Does conversion corrupt the text?                           | **No.** Conversion completes before any injection, and the converted string is correct. |
| Does per-character synthetic injection deliver text intact? | **Not always.** Corruption reproduced unattended, matching the report.                  |
| Is the loss specific to one application?                    | **Not established.** 19/20 against 20/20 is noise at this sample size.                  |
| Does pacing prevent it?                                     | **No.** One batch through 15 ms per character all failed somewhere.                     |
| Can the program detect a failed write?                      | **No.** `SendInput` reports events enqueued, not events applied.                        |
| What does a conversion cost in undo steps?                  | **One per character.** Deterministic, every time.                                       |
| Do the clipboard exclusion formats apply?                   | **Yes.** 0 of 3 against 3 of 3, differentially.                                         |

## Machine configuration

Every finding is specific to this machine. That is acceptable — the product has one user on
one machine — but it is the scope of the claim.

|                  |                                                        |
|------------------|--------------------------------------------------------|
| OS               | Microsoft Windows 11 Pro                               |
| Build            | 10.0.26200.0 (25H2)                                    |
| Toolchain        | Rust 1.97.1 `stable-x86_64-pc-windows-msvc`            |
| Session elevated | **No**                                                 |
| Target A         | Modern Notepad `11.2606.15.0` (packaged, WinUI)        |
| Target B         | Notepad++ (`C:\Program Files\Notepad++\notepad++.exe`) |

---

## What was reported

Three presses of `Shift+Pause` on the same selected input, with Ukrainian active, expecting
`Test if selected text may be transliterated back to the correct locale`:

```
Test ccccccccccccccccccck
Test oooooooooooooooooooooooooooooooooo e
Test ddddddddddddddddddddddd back eeeeeeeeeeeeeeeeeeeee
```

Identical input, three different outputs. That alone rules out a deterministic logic fault
and points at a race. The correct fragments — `Test `, `back`, the trailing ` e` — show the
conversion itself producing the right characters.

## The conversion is not implicated

`convert` runs over the whole string and returns a finished `String` before anything is typed
(`selection.rs:136`), which is then expanded into synthetic events (`inject.rs:94-99`). The
two stages are sequential and separable, and only the second is timing-dependent.

**The decisive observation came from `Ctrl+Z`, not from any probe.** Undoing a conversion
removes the replacement one character at a time. That is a direct read-out of the delivery
model, it is deterministic, and it needs no repetition to establish — unlike the corruption,
which had to be caught in the act. Two independent defects share one cause.

## Instruments

Four probes were written, all under `lexiswitch-win/examples/`:

| Probe                       | Measures                                                                                                            |
|-----------------------------|---------------------------------------------------------------------------------------------------------------------|
| `probe_selection`           | which branch `convert_selection` would take, as a ladder rather than the single `Option` the real read collapses to |
| `probe_inject`              | one attended injection at a chosen pacing, read by eye                                                              |
| `probe_inject_auto`         | unattended: launches a target, opens a fresh document, types at several pacings, reads back, compares               |
| `probe_clipboard_exclusion` | whether the Clipboard History opt-out formats are applied, checked differentially                                   |

Two harness properties were learned the expensive way and are recorded because any future
instrument of this kind needs them:

- **It must open its own document.** An editor that restores its previous session comes up
  holding the user's files, and a probe that selects-all and types will overwrite them. This
  was discovered by doing it.
- **Readback must not use UI Automation** if the targets being compared include one that
  exposes no text pattern — see finding 1. A readback that works on one target and not the
  other measures nothing.

---

## Finding 1 — text pattern availability differs between the two editors

| Target         | UI Automation text pattern                        |
|----------------|---------------------------------------------------|
| Modern Notepad | **exposed** — document readback succeeded         |
| Notepad++      | **absent** — every readback returned `UNREADABLE` |

Consequence: the two editors take *different branches* in `convert_selection`. Notepad++ has
no pattern, so it falls to the clipboard read; modern Notepad takes the automation read.

This is not the cause of the corruption — both branches write identically — but it explains
why the two applications behaved differently in the original report, and it is why the
automated harness had to move to a clipboard readback to compare them at all.

## Finding 2 — delivery is intermittently lossy

Sample corruption from a clean run, injecting the 70-character sentence:

```
expected: Test if selected text may be transliterated back to the correct locale
observed: Test if selected text may be transliterated aack eeeeeeeeeeeeeeeeeeeee
```

`back` became `aack`, followed by a 21-character repeat — the reported signature, reproduced
with no hotkey, no UI Automation read, and no modifier key held.

| Target         | Readback      | Environment                         | Settle  | Intact    |
|----------------|---------------|-------------------------------------|---------|-----------|
| Modern Notepad | UI Automation | ~9 tabs accumulated from prior runs | 600 ms  | 15/20     |
| Modern Notepad | clipboard     | fresh single instance               | 1200 ms | **19/20** |
| Notepad++      | clipboard     | fresh tab                           | 1200 ms | **20/20** |

**The first row is not a comparable measurement and is recorded as a caution.** It carries two
confounds: a settle short enough to score a slow drain as data loss, and a target polluted by
the harness's own earlier runs. It is kept because it was the run that first exposed the
effect, and because "worse under load" is itself an observation — but any rate taken from it
is wrong.

## Finding 3 — pacing is not the variable

Per-pacing results, five runs each, clean instances, clipboard readback:

| Pacing                                  | Modern Notepad | Notepad++ |
|-----------------------------------------|----------------|-----------|
| one batch — what `inject::replace` does | 5/5            | 5/5       |
| 8 characters per call, 2 ms apart       | **4/5**        | 5/5       |
| 1 character per call, 5 ms apart        | 5/5            | 5/5       |
| 1 character per call, 15 ms apart       | 5/5            | 5/5       |

The single failure occurred at an intermediate pacing while the *unpaced* batch went 5/5.
Under the polluted first-row conditions every pacing failed at a similar rate, 15 ms per
character included. Pacing is therefore not protective, and a chunked `inject::replace` would
have presented as fixed under casual testing while remaining broken.

A single `SendInput` call is not an atomic edit. The whole replacement already goes out in one
call — the call enqueues about 140 events for a 70-character sentence, which the target then
consumes individually. Batching at the sending end changes nothing about that.

## Finding 4 — a failed write is undetectable by the program

`SendInput` returns the number of events inserted into the input stream, not the number the
target acted on. `inject::replace` treats a full count as success (`inject.rs:113-116`), and
the worker reports automation-path success silently (`worker.rs:150`).

A mangled or wholly discarded write therefore presents to the user as *nothing happening*,
with no notification — which is exactly how the original report described it.

## Finding 5 — undo granularity is one step per character

The target records each injected character as its own edit. A 70-character conversion costs
roughly 70 `Ctrl+Z` presses to reverse. Unlike finding 2, this is deterministic and present on
every conversion in every application.

## Finding 6 — the clipboard exclusion formats are applied

Registered atoms on this machine, and the differential result:

```
 49923  ExcludeClipboardContentFromMonitorProcessing
 49918  CanIncludeInClipboardHistory
 49917  CanUploadToCloudClipboard

set_text (user's own content)    0 of 3 exclusion formats, text intact
set_text_excluded (generated)    3 of 3 exclusion formats, text intact
```

**Scope of this claim.** What was measured is that Windows accepts the formats and reports
them present on the clipboard. It was **not** measured that Windows Clipboard History or Cloud
Clipboard actually suppress the entry, nor that any third-party clipboard manager honours
them — those formats are advisory, and a manager is free to ignore them. The privacy property
they buy is cooperative, not structural.

---

## What these findings do not establish

- **Not an application-specific defect.** One failure in twenty against none in twenty does
  not separate the two editors. The Notepad++ result is consistent with immunity and equally
  consistent with a rate too low for twenty trials to catch.
- **Not root-caused.** The mechanism inside the target was not identified. The behaviour is
  consistent with an application consuming input asynchronously and sampling state rather than
  draining every event, but that is inference from the outside, not measurement.
- **Not stable rates.** Every figure here is n=20 per configuration, on one machine, with
  whatever else was running at the time. They establish that the loss is real and that pacing
  does not prevent it. They do not support comparing 19/20 against 20/20.

## Incidental observations worth carrying

- **An empty Notepad++ document yields `\r\n` on select-all-and-copy, not nothing.** An
  emptiness check must treat whitespace as empty or it will refuse to run.
- **Copying an empty selection leaves the clipboard untouched**, so the previous contents come
  back and an empty document reads as full. Establishing emptiness requires planting a
  sentinel first and seeing whether it survives.
- **Modern Notepad reuses one process with tabs.** Launching `notepad.exe` while an instance
  is running hands off to it and may not bring it to the foreground, which will hang a harness
  that waits for its own window.
- **Block-buffered stdout turns a stalled probe into silence**, which is indistinguishable
  from a probe that did nothing. Flush after every line.
