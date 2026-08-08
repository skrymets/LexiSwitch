# Manual checklist

Everything on this page needs a person. It is short on purpose: anything that could be driven
from a script has been, and lives in `run-matrix.ps1` or in the probes under
`lexiswitch-win/examples/`. If an item here looks automatable, say so — it probably is, and it
should not be your time.

**Where results go:** fill in the *Saw* column below and leave it. Findings get transcribed into
`docs/manual-tests.md`, which is the permanent record; this file is the working surface.

**Before starting:** one `lexiswitch.exe` running, from `target\release\`. Check for strays
first — several confusing hours in this project's history were a second instance nobody knew
about:

```powershell
Get-Process lexiswitch | Select-Object Id, Path, StartTime
```

---

## A. Needs your Windows configuration changed

Only you should touch the installed keyboard layouts. Both of these are accepted as untested
otherwise, and that is recorded — running them is an improvement, not a blocker.

| #  | Do this                                                                                                               | Expect                                                                                                                           | Saw                       |
|----|-----------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|---------------------------|
| A1 | Remove all but one keyboard layout, then start LexiSwitch                                                             | It **starts**. Tray icon appears with a warning look. Tooltip says one layout is installed and two are needed. It does not exit. | Observation 1 (see below) |
| A2 | With it still running in that state, add a second layout in Windows settings, then hover or right-click the tray icon | It notices without a restart: the icon returns to normal and a conversion works                                                  | Observation 2 (see below) |
| A3 | Put your layouts back as they were                                                                                    | —                                                                                                                                | -                         |

### Observation 1

1. Run: `Get-Process lexiswitch | Select-Object Id, Path, StartTime`
   Get: `Get-Process : Cannot find a process with the name "lexiswitch"`

2. Run: `.\target\release\lexiswitch.exe`
   `Get-Process lexiswitch | Select-Object Id, Path, StartTime`
```text
   Id Path                                                    StartTime
   -- ----                                                    ---------          
   11776 C:\projects\my\lexiswitch\target\release\lexiswitch.exe 07/08/2026 18:56:02
```

- A notification balloon appeared:
```text
LexiSwitch
Conversion needs at least two keyboard layouts, and 1 is installed. Add a second layout in Windows settings; LexiSwitch will pick it up without being restarted.
```
- Icon in the tray has a form of a yellow triangle with the exclamation mark. Tooltip says: «1 keyboard layout is installed, two are needed»
- `<Shift>+<Pause>` does nothing.

### Observation 2

3. Add another keyboard layout (UA):

- A notification balloon appeared:
```text
LexiSwitch — Shift+Pause converts the selection
Select text and press Shift+Pause to convert it.
```
- Icon in the tray has a form of a generic application icon
- `<Shift>+<Pause>` shows the conversion chooser dialog and then actually converts a selected text



---

## B. Needs your eyes

The tray's tooltip and icon cannot be captured reliably from a script, and "visibly different"
is a judgement rather than a comparison.

| #  | Do this                                                                                                                                                    | Expect                                                                                                                                                                          | Saw                                                                                             |
|----|------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------|
| B1 | Hover the tray icon in normal operation                                                                                                                    | Tooltip names **both** shortcuts, e.g. *"LexiSwitch — Win+Shift+Pause converts, Win+Ctrl+Pause changes case"*. Re-check: the wording changed when the second transaction landed | Confirmed against the one-shortcut wording; needs re-checking                                   |
| B2 | Right-click → Disable, then hover again                                                                                                                    | Icon **visibly** changes. Tooltip says disabled. It does not look like an error — being switched off is your choice, not a fault                                                | Expected behavior confirmed                                                                     |
| B3 | Re-enable. In a terminal run `cargo run -p lexiswitch-win --example probe_hold_hotkey -- "Win+Shift+Pause" 90`, then restart LexiSwitch and hover its icon | Icon looks like something needs attention, and is distinguishable from both B1 and B2. Tooltip names the combination that is unavailable                                        | *(restated: a second LexiSwitch no longer starts, so the conflict now needs a stand-in holder)* |
| B4 | Right-click the tray icon → Diagnostics                                                                                                                    | A balloon naming the state, the layouts installed, the configuration path and whether logging is on. No selected text or conversion results anywhere in it                      | Expected behavior confirmed                                                                     |
| B5 | Stop the holder (Ctrl+C in its terminal, or let it time out), then hover the icon again                                                                    | The conflict clears without a restart                                                                                                                                           |                                                                                                 |

---

## C. Needs your judgement

The part no checklist covers: whether it is actually pleasant to use.

| #  | Do this                                                                                | Watch for                                                                                                                                  | Saw                                                                             |
|----|----------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| C1 | Use it normally for a day in whatever you actually work in                             | Anything surprising, slow, or in the way. Especially: a chooser that appears when you did not want one, or a conversion you did not expect | Let's postpone for the later times.                                             |
| C2 | Try it in an application not in the matrix — Telegram, Word, Outlook, an IDE           | Conversion works, or fails **visibly** rather than silently. A silent nothing is the bug worth reporting                                   | It does work as expected in<br/> Microsoft Word. <br/> See Observation 3 below. |
| C3 | Interrupt a conversion mid-flight: press the shortcut, then click away before choosing | Nothing is modified. The clipboard is what it was                                                                                          | Expected behavior confirmed                                                     |

### Observation 3

In IntelliJ Idea (IDE)

1. I have n XML editor (text editor) I have text «БщкпфтшяфешщтЮ» which is «<organization>» but in UA layout.
2. I select this text and press <Shift>+<Pause> - and get the balloon:
```text
«Selection could not be read nothing is selected; nothing was changed»
```
3. I make some random cursor (text cursor in the editor) movements, select the same text, press `<Shift>+<Pause>` and now I get a translation chooser dialog and can translate the text.
4. I repeat the procedure several times, and I randomly either get the chooser dialog on pressing `<Shift>+<Pause>`, or balloon with «...nothing is selected...». Randomly. I don't see any system in that.

Lets just write down this as a bug, but not focus on fixing it at this time.

---

## What is already covered, so you do not have to

Recorded in `docs/manual-tests.md` with the evidence:

- Conversion in Notepad, Notepad++, Chrome and Edge — apply, apply-and-switch, cancel, no
  conversion found, a read-only field, and a collapsed caret. 25 cases, run against the release
  build.
- A shortcut held by another application: start-up survives it, the tray converts, a replacement
  is recorded from the keyboard, and the conflict clears when the holder goes away.
- One `Ctrl+Z` reverting a conversion; the prior clipboard restored on every path.
- An hour idle: no leak, no outbound connection.
- Enable/disable gating the hotkey — you confirmed this on 2026-08-07.

---

## Group D — the case window (added 2026-08-08)

Everything here is a *rendering* claim. The behaviour behind each one is already asserted by
`shell-checks.ps1 -Only Case`; what a script cannot say is whether it looks right. Open it with
`Win+Ctrl+Pause` over a selection, or from the tray's **Change case…**.

Use this text, which exercises every column at once:

```
hello,   World 42!  Bye.
```

| #   | Do this                                                  | Expect                                                                                                                                                                                                        | Saw                                                                                                                                         |
|-----|----------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------|
| D1  | Look at the list without pressing anything               | Three groups separated by rules, in this order: the plain four (`As typed`, `Invert case`, `lowercase`, `UPPERCASE`), then the eight identifier styles, then `First word capitalized` and `Words Capitalized` | Confirmed                                                                                                                                   |
| D2  | Look at rows whose result equals the original            | Drawn dim, **still present and still selectable** — never removed                                                                                                                                             | Confirmed                                                                                                                                   |
| D3  | Look at the toggle strip                                 | Three labelled rows: `Strip 1/2`, `Spaces 3/4`, `Lines 5/6`. The two pairs read as pairs                                                                                                                      | Confirmed                                                                                                                                   |
| D4  | Press `3`, then `4`                                      | The `3` box clears as `4` sets. They never both show `[x]`                                                                                                                                                    | Confirmed                                                                                                                                   |
| D5  | Select `snake_case`, then look at toggles 3 and 4        | Dim — they cannot change what `snake_case` produces                                                                                                                                                           | Dim, but 3/4 still toggle. **By design** — dim means "no effect on *this* style", not disabled; the option is global and must stay settable |
| D6  | Press `1` and watch every row, not just the selected one | Every preview updates at once, so the styles stay comparable                                                                                                                                                  | Confirmed                                                                                                                                   |
| D7  | Select a long result and press `Space`                   | Expands, wraps, scrolls with `↑↓`, and shows `line n of m`; nothing is written                                                                                                                                | Confirmed. Whole dialog blinked while scrolling — **fixed**, background erase disabled on repaint. Re-check                                 |
| D8  | Read the footer                                          | Every binding named: `↑↓`, `1-6`, `Space`, `Ctrl+R`, `Enter`, `Esc`                                                                                                                                           | Confirmed                                                                                                                                   |
| D9  | Set display scale to 150% and reopen                     | The whole list is reachable — either it all fits, or it scrolls and keeps the selected row visible. Nothing is cut off at the bottom                                                                          | Confirmed it scrolls, but no visible scrollbar — **fixed**, indicator added. Re-check                                                       |
| D10 | Apply a style three times, reopen                        | An `Often used` heading with that style on top; the full list below still holds it in its declared position                                                                                                   | Confirmed Confirmed                                                                                                                         |

**D9 is the one worth doing properly.** The window is sized to its content and then clamped to
the work area, so at a large scale the clamp is what decides whether the last style is
reachable. It is the only item here that could hide a real defect rather than an ugly one.

---

## Group E — copying a result without applying it (added 2026-08-08)

The behaviour is asserted by `shell-checks.ps1 -Only Copy`, including the two that matter most:
a copy survives cancelling, and it survives committing a *different* option. What a script
cannot answer is the one thing the `H` toggle exists for — whether Windows Clipboard History
actually honoured the opt-out. `IsClipboardFormatAvailable` says the exclusion was **requested**;
only the history panel says it was **respected**.

Open the history panel with `Win+V` and keep it in view.

| #  | Do this                                                                    | Expect                                                                                                               | Saw |
|----|----------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------|-----|
| E1 | Open either window, look at the footer                                     | `Ctrl+C / Ctrl+Ins copy without applying` and the current state of `H`                                               |     |
| E2 | Select an option, press `Ctrl+C`                                           | The footer changes to say it was copied                                                                              |     |
| E3 | Press `Esc`, then paste somewhere                                          | The transformed text, and the original selection untouched                                                           |     |
| E4 | With `H` **off** (the default), copy, then check `Win+V`                   | The copied text is **not** in Clipboard History                                                                      |     |
| E5 | Press `H` so it reads "keeps it in clipboard history", copy, check `Win+V` | The copied text **is** in the history                                                                                |     |
| E6 | Copy with `H` off, then press `H` **after** copying, then check `Win+V`    | It appears — flipping the toggle re-copies under the new answer rather than leaving the two disagreeing              |     |
| E7 | Copy one option, then `Enter` on a **different** one, then `Win+V`         | The document has the applied option; the history shows the copied one, not the clipboard from before the transaction |     |
| E8 | `Ctrl+Ins` instead of `Ctrl+C`                                             | Identical behaviour                                                                                                  |     |

**E4 and E5 are the pair that matters.** Everything else here is confirmed by script; these two
are the only evidence that the opt-out does anything, and the script deliberately does not claim
they passed.
