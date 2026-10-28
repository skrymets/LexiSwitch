## 1. The style, test-first

- [x] 1.1 Failing tests in `lexiswitch-core/tests/casing.rs`: add `Style::Abbreviation` to `every_style_against_one_phrase` (`hello World 42 Bye` → `HW4B`); every label from the proposal's examples abbreviates as listed (`As typed` → `AT` … `kebab-lowercase` → `KL` … `Words Capitalized` → `WC`); leading punctuation is skipped; a word of punctuation alone contributes nothing; `Windows 11` → `W1`, and `W` with digits stripped; Ukrainian abbreviates and uppercases; `HTTPServer` → `H`.
- [x] 1.2 Failing tests for lines: two lines give two abbreviations with the break kept byte for byte (including `\r\n`); `lines: join` and `lines: remove` each give one abbreviation; the space options do not affect the abbreviation (`toggle_affects` is false for both).
- [x] 1.3 Failing tests for the list: `Style::GROUPS` has four groups, the last is `[Style::Abbreviation]`, and its label is `ABBR` and id `abbreviation`. The existing uniqueness and groups-concatenate-to-`ALL` tests cover the rest.
- [x] 1.4 Implement in `lexiswitch-core/src/casing.rs`: the variant, `ALL` and `GROUPS`, `label`, `id`, admission to `is_identifier` with its doc comment widened, the empty delimiter, and the per-word reduction in `case_word`. Update the module doc's grouping sentence on `Style`.
- [x] 1.5 `cargo test -p lexiswitch-core` green.

## 2. The window and the docs

- [x] 2.1 Confirm `lexiswitch-win/src/casewindow.rs` needs no change: it builds from `Style::GROUPS`, so the fourth rule and row appear on their own. `cargo build` for the workspace.
- [x] 2.2 `README.md`: add an **Abbreviation** row to the style table.
- [x] 2.3 `docs/testing/manual-checklist.md` D1: four groups, the last holding `ABBR`; mark D1 and D9 as needing a re-check.

## 3. Verification

- [x] 3.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`, `cargo build --release` under 5 MB with no warnings.
- [ ] 3.2 Manual: open the case window on `First word capitalized` and see `ABBR` alone in a fourth group showing `FWC`; commit it; repeat D9 at 150 % scale. Record in `docs/manual-tests.md`.

## 4. Revised abbreviation rules

The rules changed after group 1 landed: numbers are kept whole, capitals are kept as already
abbreviated, and any non-alphanumeric separates words for `ABBR`. This group supersedes the
`W1`, `H` and leading-punctuation expectations of 1.1 and the identifier-path approach of 1.4.

- [x] 4.1 Failing tests: `Windows 11` → `W11` (`W` with digits stripped); `All 14 of your examples` → `A14OYE`; `hello World 42 Bye` → `HW42B` in the every-style table; `HTTPServer` → `HTTPS`; `ABBR` → `ABBR` and `UPPERCASE` → `UPPERCASE`, both with `changes == false`; `SCREAMING_SNAKE_CASE` → `SSC`, `KEBAB-UPPERCASE` → `KU`; `openspec/changes/add-abbreviation-case-style` → `OCAACS`; `/opsx:archive` → `OA`; `привіт світ → ПС` → `ПСПС`. Update the every-label test for `UPPERCASE` and `ABBR`.
- [x] 4.2 Failing test: other styles' segmentation is untouched — `a/b` under `snake_case` is still `ab`, `HTTPServer` still `httpserver`.
- [x] 4.3 Keep passing: per-line output with `\n` and `\r\n` kept, `lines: join`/`remove` giving one abbreviation, space options without effect; add punctuation option without effect.
- [x] 4.4 Implement `abbreviate` in `lexiswitch-core/src/casing.rs`, dispatched from `render`; take `Abbreviation` out of `is_identifier`, `delimiter` and `case_word`, and restore the predicate's doc comment.
- [x] 4.5 `README.md`: reword the Abbreviation row for the new rules.
- [x] 4.6 Rerun the full gate of 3.1.
