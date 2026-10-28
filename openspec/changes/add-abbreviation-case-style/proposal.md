## Why

Turning a phrase into its abbreviation — `First word capitalized` into `FWC`, `snake_case` into
`SC` — is a re-casing of selected text that the case window cannot currently do, and it is
built from exactly the parts the window already has: a word sequence, a per-word transform, and
a join. The style list is fixed by requirement, so adding one means amending that requirement
rather than slotting a row in.

## What Changes

- Add a fifteenth case style, labelled `ABBR`, that reduces each word of the selection to its
  first character, uppercased, with nothing between them — keeping whatever in the text is
  already abbreviated.
- Give it a fourth group of its own at the end of the list. It is neither an identifier style
  nor a sentence style, and a group boundary is what tells the user so.
- For `ABBR` only, any character that is not a letter or a digit separates words, so a path
  abbreviates by its parts: `openspec/changes/add-abbreviation-case-style` gives `OCAACS`,
  `/opsx:archive` gives `OA`. Other styles' word boundaries do not change.
- Numbers are kept whole: `Windows 11` gives `W11`, `All 14 of your examples` gives `A14OYE`.
  The digits option removes them, giving `W`.
- Capitals are already abbreviated and are kept: `HTTPServer` gives `HTTPS`, `ABBR` gives `ABBR`,
  `привіт світ → ПС` gives `ПСПС`. The exception is a line written in capitals throughout with
  more than one word, which is shortened word by word: `SCREAMING_SNAKE_CASE` gives `SSC`.
  A single capitalised word, `UPPERCASE` included, is kept whole.
- A multi-line selection yields one abbreviation per line, line breaks kept exactly as written,
  as the identifier styles already do. `lines: join` or `lines: remove` fuses them into one.
- The new style's usage identifier is `abbreviation`. Existing stored counts are unaffected.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `case-conversion`: the fixed style list gains `ABBR` in a fourth group, and a new requirement
  defines what the abbreviation style produces.

## Impact

- `lexiswitch-core/src/casing.rs`: new `Style` variant, `ALL` and `GROUPS` grow, label and id,
  and an assembly path of its own for the abbreviation.
- `lexiswitch-core/tests/casing.rs`: the every-style table and abbreviation cases.
- `lexiswitch-win/src/casewindow.rs`: no code change expected — it draws `Style::GROUPS`, and
  the height and scrolling already follow the line count. Verified by eye, not assumed.
- `README.md` style table and `docs/testing/manual-checklist.md` D1 name the groups and need the
  fourth.
- No new dependency, no Win32, no change to what is stored beyond one more possible identifier.
