# Release notes

Versions follow [Semantic Versioning](docs/adr/0016-semantic-versioning.md). While the version
is `0.x`, a MINOR release may change behavior you rely on and says so under **Changed**.

## 0.2.0

### Added

- **Transliteration.** `Win+Alt+Pause` writes the selected Cyrillic text in Latin letters, with one row per ruleset to choose from.
- Built-in rulesets: Ukrainian (Resolution No. 55) and russian (ICAO Doc 9303).
- Your own rulesets, as TOML files in `%APPDATA%\LexiSwitch\transliteration\`, are picked up without a restart.
- A ruleset file with a mistake is reported with its line, and its last working version stays in use.
- `transliteration.example.toml` in the release zip: the Ukrainian ruleset with every part of the format explained.
- Tray menu: **Transliterate selection…**, **Change transliteration shortcut…** and **Open transliteration folder**.
- `[hotkeys] transliterate` setting, default `Win+Alt+Pause`.
- **ABBR** case style: the first character of each word, keeping capitals and numbers whole (`HTTPServer` → `HTTPS`, `Windows 11` → `W11`).
- The running version is shown at the top of the tray menu and in Diagnostics, and the executable's File Properties → Details shows it too.
- Each release also publishes the program on its own, as `lexiswitch-<version>.exe`, with a SHA-256 checksum.

### Changed

- A shortcut you never set that clashes with the one you did is now left without a combination and reported in the tray. Previously, the whole configuration file was rejected.
- Release files are named by a version without the `v`: `lexiswitch-0.2.0-x86_64-pc-windows-msvc.zip` rather than `lexiswitch-v0.1.0-…`.

### Fixed

- Configuration errors named the line before the faulty one when the problem was a key at the start of its line.
- Four tray notifications had a long run of spaces in the middle of a sentence: shortcut unavailable, shortcuts clash, shortcut not saved, and clipboard content lost.

## 0.1.0

First release.

### Added

- **Layout conversion.** `Win+Shift+Pause` lists the selected text as it would read in every pair of installed keyboard layouts, each row saying how much of it would stay unchanged.
- `Ctrl+Enter` applies a conversion and switches the typing layout to the one it converted to.
- **Case conversion.** `Win+Ctrl+Pause` lists fourteen styles (plain, identifier, and prose), always in the same order. A style that would change nothing is dimmed.
- **Often used** puts the three styles you apply most at the top. `Ctrl+R` resets the counts.
- Six strip options (punctuation, digits, spaces, line breaks), applied before the case style and toggled with `1`–`6`.
- `Space` shows the whole result, with the part that would change highlighted.
- `Ctrl+C` copies a result without applying it. The copy is kept out of Clipboard History unless you press `H`.
- Nothing is written until `Enter`. `Esc` leaves everything as it was.
- Works on the selection in any application: it reads it through UI Automation or falls back to the clipboard and restores the clipboard's text afterward (`allow_clipboard_fallback`).
- Tray icon with three states (working, switched off, needs attention) and a menu with every command, enable/disable, and Diagnostics.
- Shortcuts can be changed by pressing the new combination in the tray. A shortcut another program holds is reported, and the commands stay in the tray menu.
- Optional `config.toml`, reloaded without a restart. A file that does not parse is rejected whole, and the tray names the line.
- One instance per session: starting a second makes the running one announce itself.
- No keystroke observation and no network capability, and nothing you convert is ever written to disk.
- Released as a zip with an SHA-256 checksum.
