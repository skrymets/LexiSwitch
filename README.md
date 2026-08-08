# LexiSwitch

![Application Icon](media/icons/windows/main/icon_128.png)

**Fix text you typed under the wrong keyboard layout — and change the case of anything you
select — without leaving the keyboard.**

An offline tray utility for Windows 11. No account, no network, no installer.

![The layout chooser](docs/screenshots/chooser.png)

---

## What it's for

Two small problems that cost a surprising amount of time.

**You typed in the wrong layout.** You meant to write Ukrainian, English was still active, and
what came out was `{jxe ltoj d nt,t pfgbnfnb/`. Retyping it is the usual answer. Select it,
press `Win+Shift+Pause`, press `Enter` — it becomes `Хочу дещо в тебе запитати.`

**You need the text in a different case.** A sentence has to become `snake_case` for a variable
name, a shouted line has to come down to lowercase, a heading needs Every Word Capitalized.
Select it, press `Win+Ctrl+Pause`, pick from the list.

Both work in whatever you happen to be writing in — an editor, a browser field, a chat box —
because they operate on the selection rather than on a document they had to open.

---

## Two modes, two shortcuts

They are deliberately separate. Any text can be re-cased and any text has layout candidates, so
one merged list would be about half irrelevant every time.

### `Win+Shift+Pause` — fix the layout

![The layout chooser](docs/screenshots/chooser.png)

Every pair of installed layouts is offered: what the text would be if you had meant to type it
in Ukrainian, in German, in English. Each row shows the result and how much of the text it would
leave unchanged, so the right one is usually obvious at a glance — a candidate that changes
almost nothing is almost certainly not what you meant.

`Ctrl+Enter` applies the conversion **and switches your typing layout** to the one it converted
to, which is normally the next thing you were going to do anyway.

### `Win+Ctrl+Pause` — change the case

![The case window](docs/screenshots/case.png)

Fourteen styles, always all of them, always in the same order: a style that would change nothing
is dimmed rather than dropped, so the row you are aiming for is where it was last time.

| Category        | Conversions                                                                                                                        |
|-----------------|------------------------------------------------------------------------------------------------------------------------------------|
| **Plain**       | As typed · Invert case · lowercase · UPPERCASE                                                                                     |
| **Identifiers** | camelCase · snake_case · dot.case · kebab-lowercase · PascalCase · SCREAMING_SNAKE_CASE · Capitalized_Snake_Case · KEBAB-UPPERCASE |
| **Prose**       | First word capitalized · Words Capitalized                                                                                         |

**Often used** at the top holds the three styles you apply most, so the ones you actually use
are a keystroke away. `Ctrl+R` forgets them and starts counting again.

The six numbered checkboxes strip things out before the style is applied — punctuation, digits,
spaces, line breaks. `1` through `6` toggle them.

![Strip options](docs/screenshots/case-toggles.png)

Removing spaces does not destroy word boundaries: `Words Capitalized` with spaces removed gives
`HelloWorld`, not `Helloworld`. Each line keeps its own identity unless you ask for the lines to
be joined.

---

## What both windows do

**Nothing is written until you press `Enter`.** Opening the window, moving through it, and
turning options on and off change nothing in your document. `Esc` leaves everything exactly as
it was.

**`Space` shows the whole result** when the one-line preview is too short to judge by. The part
that would change is highlighted.

![The expanded view](docs/screenshots/case-expanded.png)

**`Ctrl+C` copies a result without applying it.** Pick an option, copy it, press `Esc` — your
selection is untouched and the transformed text is on the clipboard, ready to paste somewhere
else. By default that copy is kept out of Windows Clipboard History; `H` lets it in for this one
copy.

---

## Getting started

**You need** Windows 11 and **at least two keyboard layouts installed** — with only one there is
nothing to convert between, and LexiSwitch says so and stops rather than sitting in the tray
doing nothing.

1. Run `lexiswitch.exe`. There is no installer and nothing is written outside `%APPDATA%`.
2. It goes to the notification area and tells you its shortcuts.
3. Select some text and press one of them.

To start it with Windows, put a shortcut to it in
`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`.

Only one copy runs at a time. Starting a second makes the first one announce itself instead.

### The tray icon

Right-click it for everything the program can do:

![The tray menu](docs/screenshots/tray-menu.png)

- **Convert selection** and **Change case…** — the same two transactions, for when a shortcut is
  unavailable
- **Disable LexiSwitch** — the shortcuts stop converting until you enable it again
- **Change conversion shortcut…** / **Change case shortcut…** — press the combination you want
- **Diagnostics** — what state it is in and why
- **Reload configuration** / **Open configuration file**
- **Quit**

The icon itself says which state it is in, without hovering for the tooltip:

![The three tray icons](docs/screenshots/tray-states.png)

"Needs attention" means a shortcut another program has taken, a configuration file it refused to
load, or too few keyboard layouts to convert between. Which one it is, it tells you.

---

## Keys

| | Layout window | Case window |
|---|---|---|
| Open | `Win+Shift+Pause` | `Win+Ctrl+Pause` |
| Move | `↑` `↓` | `↑` `↓` `PgUp` `PgDn` `Home` `End` |
| Apply | `Enter` | `Enter` |
| Apply and switch typing layout | `Ctrl+Enter` | — |
| Show the whole result | `Space` | `Space` |
| Strip options | — | `1`–`6` |
| Forget "Often used" | — | `Ctrl+R` |
| Copy without applying | `Ctrl+C` or `Ctrl+Ins` | `Ctrl+C` or `Ctrl+Ins` |
| Allow that copy into Clipboard History | `H` | `H` |
| Close, changing nothing | `Esc` | `Esc` |

---

## Settings

Everything is optional. Right-click the tray icon → **Open configuration file** and it creates a
commented example at `%APPDATA%\LexiSwitch\config.toml` on first use. Changes are picked up
without a restart.

```toml
[general]
enabled = true                        # whether the shortcuts convert

[hotkeys]
convert_selection = "Win+Shift+Pause"
change_case       = "Win+Ctrl+Pause"

[layouts]
candidates = []                       # e.g. ["uk-UA", "en-US"] to fix the order

[selection]
allow_clipboard_fallback = true       # read the selection via the clipboard where the
                                      # accessible-text route does not work

[chooser]
expanded_lines = 10                   # how tall the expanded view is

[logging]
enabled = false                       # event kinds and lengths only, never text
```

A configuration file that does not parse is **rejected whole**: the settings you were already
running stay in force, and the tray tells you which line it choked on. There is no half-applied
state to get stuck in.

One other file is written, `%APPDATA%\LexiSwitch\state.toml`, and it holds only how many times
each case style has been used — style names and integers, nothing else. Delete it, or press
`Ctrl+R` in the case window, and the counting starts again.

---

## What it does not do

This matters more than a feature list, so it is stated as a property rather than a promise.

**It never watches you type.** There is no keystroke monitor of any kind — not a low-level hook,
not Raw Input, not anything. It has no idea what you are typing and no way to find out. It sees
exactly one thing: the text you selected, at the moment you pressed a shortcut, and only then.
(The earlier design did observe typing through an API that structurally cannot alter a
keystroke; that whole capability was removed rather than made safer.)

**It cannot reach the network.** There is no telemetry, no update check, no crash reporting, and
no code that could do any of them. The whole dependency tree is `serde`, `toml` and the Windows
API bindings; network-capable crates are banned by `cargo-deny` as part of the build, not as a
policy someone has to remember.

**Nothing you convert reaches disk.** Not the configuration, not the log, not a crash dump.
Logging is off by default and records event kinds and lengths (`converted_word len=7`), never
content.

**A password field is only ever at risk if you select one yourself.** There is no detection to
trust here and none is needed: the program reads what you selected, when you asked, and nothing
else. Earlier versions did detect password fields — that was there to protect a keystroke buffer
that no longer exists, and its coverage inside browsers was measurably poor, so the thing it
guarded was removed instead ([ADR-0011](docs/adr/0011-no-keyboard-observation.md)).

Under 1.1 MB, no runtime, no background service.

---

## Known limits

- **The clipboard is borrowed** when an application does not expose its selection through the
  accessible-text interface (most editors do; some do not). The previous contents are put back,
  but only text formats are restored — an image on the clipboard survives, a niche private
  format may not. Turn `allow_clipboard_fallback` off to forbid this entirely, at the cost of
  those applications.
- **Some applications hide their selection.** Where neither the accessible-text route nor the
  clipboard fallback can read it, LexiSwitch says the selection could not be read rather than
  guessing at one.
- **A modifier you are still physically holding** can occasionally swallow the keystrokes that
  apply the result — roughly one attempt in five in the worst-behaved applications. Release the
  shortcut before pressing `Enter`.

---

## Building it yourself

```powershell
cargo build --release        # target\release\lexiswitch.exe
cargo test --workspace
```

Rust 1.97 or newer, MSVC toolchain, and — for the full test suite — the `en-US`, `uk` and
`de-DE` keyboard layouts installed, because the layout tables are read from Windows rather than
shipped, and German is the only one of the three with dead keys.

[`docs/environment.md`](docs/environment.md) covers the rest of the setup.
[`docs/adr/`](docs/adr/README.md) records why the design is the way it is, including the
alternatives that were rejected; [`docs/manual-tests.md`](docs/manual-tests.md) records what has
actually been verified by hand, and what broke on the way.

## Known Issues

### ⚠️ [false-positive] Release v0.1.0 is flagged as malicious by VirusTotal :) 
- This app's released binary v0.1.0 (`71d58e1dc3cfdea847720fc9d135202d33f032992b061903af41718c76c39a3a`) is treated as malware by 3/71 antivirus scanners on [.exe @ VirusTotal](https://www.virustotal.com/gui/file/71d58e1dc3cfdea847720fc9d135202d33f032992b061903af41718c76c39a3a/detection).
- At the same time, only 1/67 security vendor flagged its `.zip` file (`e226f982751b750cb09e5a3c6fa30719783e10a69761edde3f6497c75824ca75`) as malicious [.zip @ VirusTotal](https://www.virustotal.com/gui/file/e226f982751b750cb09e5a3c6fa30719783e10a69761edde3f6497c75824ca75/detection).

> 🟢 **Yep!**  
> This is why **open-source is good** – it allows anyone to inspect the code and verify what exactly it does, and ensure that it does not contain any malicious code.
