## 1. Policy

- [x] 1.1 Write `docs/adr/0017-transliteration-third-transaction.md` and add its row to `docs/adr/README.md`: a third transaction (extending ADR-0014), TOML rulesets with one `[map]` and a `[word_start]` overlay, compile at load with no disk cache (the cache, its hash invalidation and rkyv/bincode considered and rejected, with the reasons), a hand-written trie over `aho-corasick`, and the defaulted-shortcut collision rule.

## 2. The engine, test-first (`lexiswitch-core`)

- [x] 2.1 Failing tests in `lexiswitch-core/tests/translit.rs` against a small inline ruleset: longest match over `з`/`г`/`зг` (`розгон` → `rozghon`) whatever order the entries are written in; unmatched characters pass through (`Київ 2024, Kyiv!`); empty value drops; `[word_start]` used at text start, after a space, after a hyphen and after a digit, and *not* after a dropped apostrophe.
- [x] 2.2 Failing tests for derived casing: `Щедрий` → `Shchedryi`, `ЩУКА` → `SHCHUKA`, `ЗГА` → `ZGHA` (two-letter match in capitals), `Я` → `Ya`, `ЯР` → `YAR`, last letter of a capitals word (`КИЇВ` → `KYIV`), a lowercase word unchanged in case.
- [x] 2.3 Failing tests for `fallback`: an unmatched Cyrillic letter becomes the fallback; with no fallback it passes through; Latin letters, digits and punctuation never take the fallback; the untransliterated count counts unmatched Cyrillic letters either way.
- [x] 2.4 Implement `lexiswitch-core/src/translit.rs`: the compiled `Ruleset` (trie with sorted children, `anywhere`/`word_start` values), `apply(&Ruleset, &str) -> Transliterated { text, untouched }`, word-start by looking back past dropped characters, derived casing. Register the module in `lib.rs`.
- [x] 2.5 Failing tests for parsing and validation, each asserting the reported line: unparseable TOML; bad `id`; malformed `source`/`target` tag; empty key; key with a capital letter; unknown top-level key; unknown table (`[wordstart]`); a value that is not a string.
- [x] 2.6 Implement `Ruleset::parse(file_name, text)` over `toml::Table`, reusing the key-to-line helper from `config.rs` (move it to a shared place in the core if it is private).
- [x] 2.7 Failing tests: the full example table of Resolution No. 55 under the built-in Ukrainian ruleset (Алушта, Борщагівка, Вишгород, Гадяч, Згорани, Ґалаґан, Дон, Рівне, Єнакієве, Житомир, Закарпаття, Медвин, Іршава, Їжакевич, Йосипівка, Олексій, Київ, Лебедин, Михайлівка, Ніжин, Одеса, Полтава, Решетилівка, Суми, Тетерів, Ужгород, Фастів, Харків, Біла Церква, Чернівці, Шостка, Гоща, Короп'є, Юрій, Корюківка, Яготин, Костянтин, Знам'янка, Феодосія…) and the ICAO Doc 9303 table under the built-in russian one. Both built-ins parse.
- [x] 2.8 Write `lexiswitch-core/rulesets/uk-kmu55.toml` and `ru-icao.toml` (dropping `'`, `’` and `ʼ` in the Ukrainian one), embedded with `include_str!` behind `translit::builtins()`. Tests from 2.7 pass.
- [x] 2.9 Failing tests for the set: `Rulesets::build(builtins, user_files, previous)` orders built-ins first then users by name; a user file with a built-in's id takes its place; two user files with one id reject the later one by file name; a failing file keeps the version from `previous` loaded from the same file name and reports the error; a failing file with no previous version is reported and absent.
- [x] 2.10 Implement `Rulesets`.
- [x] 2.11 Measure parsing and compiling both built-ins (a release-mode test or example printing the elapsed time) and record the number in `design.md` under "Compile at load".

## 3. Configuration and state

- [x] 3.1 Failing tests in `tests/config.rs`: `hotkeys.transliterate` defaults to `Win+Alt+Pause`; empty is an error with its line; all three written and two equal is an error naming the line; `change_case = "Win+Alt+Pause"` with `transliterate` absent loads, and reports `transliterate` as defaulted-and-colliding; an old file without the key loads.
- [x] 3.2 Implement: the key, recording which hotkey keys were present in the file, and the defaulted-collision outcome on `Config`.
- [x] 3.3 Failing tests in `tests/state.rs`: `Shortcuts` with three members; `Shortcut::Unbound` is named in `summary()`/`explanation()` with the combination it would have used and which shortcut holds it; it needs attention but is not reported as another application's conflict; `suggested_shortcuts` excludes all three bindings.
- [x] 3.4 Implement the state changes.
- [x] 3.5 Document `transliterate` in `config.example.toml`, including what happens when its default is already bound.

## 4. Windows side (`lexiswitch-win`)

- [x] 4.1 `hotkey.rs`: `HotkeyId::Transliterate` with its own id and config key.
- [x] 4.2 `translitwindow.rs`: a list window modelled on `chooser.rs` — rows of name, `source → target`, `preview::compact` of the result, "N letters left" when non-zero; dimmed rows for no effect; first effective row preselected; arrows, `PgUp`/`PgDn`/`Home`/`End`, `Space` expanded view, `Ctrl+C`/`Ctrl+Ins` and `H`, `Enter`, `Esc`; footer naming the keys. Every `unsafe` block carries its SAFETY comment. Committing a dimmed row cancels.
- [x] 4.3 `tray.rs`: `MenuCommand::Transliterate`, `TransliterateShortcut`, `OpenTranslitFolder`, in the menu beside the case items.

## 5. Wiring (`lexiswitch`)

- [x] 5.1 `configwatch.rs`: `translit_dir()`, a listing snapshot of `*.toml` (name, size, modified), and loading changed listings into `Rulesets::build` with the previous set; poll it on the existing tick.
- [x] 5.2 `worker.rs`: `Job::Translit` carrying an `Arc` of the current `Rulesets`; begin a `Transaction`, compute every row, show the window, commit or cancel, report outcomes as the other two jobs do.
- [x] 5.3 `main.rs`: register the third hotkey with the collision outcome folded into `Shortcuts`; dispatch the hotkey and tray command; the recorder for the third shortcut; Open transliteration folder (create the directory, open it in Explorer, write no file); ruleset errors into the tray state and a balloon naming file and line; Diagnostics lists loaded rulesets by id.
- [x] 5.4 `lexiswitch-win/examples/probe_tray.rs` and any other caller of changed signatures updated.

## 6. Documentation and packaging

- [x] 6.1 `packaging/transliteration.example.toml`: the Ukrainian ruleset with every part of the format explained in comments, including the apostrophe trade-off and how to override a built-in.
- [x] 6.2 `.github/workflows/release.yml` Package step stages the example file into the zip; `packaging/INSTALL.txt` lists it and describes the folder.
- [x] 6.3 `README.md`: a third mode section (shortcut, window, built-ins, folder), the keys table gains a column, tray list and settings block updated.
- [x] 6.4 `docs/testing/manual-checklist.md`: Group G — the transliteration window.
- [x] 6.5 `CLAUDE.md`: "Two transactions, two front doors" becomes three, naming `translitwindow.rs` and `translit.rs` and that they do not touch the other two.

## 7. Verification

- [x] 7.1 `openspec validate add-transliteration --strict`.
- [x] 7.2 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`, `cargo build --release` under 5 MB with no warnings.
- [x] 7.3 Package dry run with `TAG=v0.3.0`: the zip contains `transliteration.example.toml`.
- [ ] 7.4 Manual, Group G: `Щедрий Знам'янка ЩУКА` in Notepad gives `Shchedryi Znamianka SHCHUKA` under the Ukrainian row; `Esc` changes nothing; Latin-only text dims every row; a broken user file is reported with file and line while the others still work; an edited user file is picked up within a few seconds; with `change_case = "Alt+Win+Pause"` and no `transliterate`, the program starts and says transliteration has no shortcut. Record in `docs/manual-tests.md`.
