## MODIFIED Requirements

### Requirement: The offered styles are a fixed, grouped list

The system SHALL offer exactly these styles, in exactly this order, in four groups:

1. `As typed`, `Invert case`, `lowercase`, `UPPERCASE`
2. `camelCase`, `snake_case`, `dot.case`, `kebab-lowercase`, `PascalCase`,
   `SCREAMING_SNAKE_CASE`, `Capitalized_Snake_Case`, `KEBAB-UPPERCASE`
3. `First word capitalized`, `Words Capitalized`
4. `ABBR`

The group boundaries SHALL be visible. The list SHALL NOT be reordered, filtered, extended or
shortened in response to the selected text or to any option the user sets, so that a style
occupies the same position on every invocation.

`As typed` SHALL apply no case change, so that the strip options of the following requirement
are usable without one.

#### Scenario: User opens the window twice with different text

- **WHEN** the user invokes case conversion on one selection and then on a completely different one
- **THEN** the same styles appear in the same order in the same groups both times

#### Scenario: User applies a style with no case change

- **WHEN** the user commits `As typed` with at least one strip option set
- **THEN** the selection is replaced with the stripped text and no letter changes case

#### Scenario: The abbreviation style is offered

- **WHEN** the user invokes case conversion on any selection
- **THEN** `ABBR` is the last style, alone in a fourth group after `Words Capitalized`

## ADDED Requirements

### Requirement: The abbreviation style keeps what already abbreviates

For `ABBR`, every character that is neither a letter nor a digit SHALL end a word, in addition
to the boundaries every style uses. This SHALL NOT change where any other style finds a word
boundary.

Each line SHALL be abbreviated on its own, and within a line:

- If the line contains no lowercase letter and more than one word, each word SHALL contribute
  its first character and every digit after it, so that text written in capitals throughout is
  shortened word by word.
- Otherwise each word SHALL contribute its first character, uppercased, followed by every
  uppercase letter and every digit after it, so that capitals already present — an acronym in
  mixed text, or a single capitalised word — are kept as already abbreviated.

The contributions SHALL be joined with nothing between them. The digits option SHALL apply
before any character is taken, so that while it is set digits contribute nothing.

A selection spanning several lines SHALL yield one abbreviation per line, separated by the line
breaks exactly as they were written. With `lines: join` or `lines: remove` set, the whole
selection SHALL be abbreviated as one line.

#### Scenario: Prose is abbreviated

- **WHEN** the selection is `First word capitalized` under `ABBR`
- **THEN** the result is `FWC`

#### Scenario: Identifiers are abbreviated by their parts

- **WHEN** the selections `camelCase`, `kebab-lowercase` and `Capitalized_Snake_Case` are each converted under `ABBR`
- **THEN** the results are `CC`, `KL` and `CSC`

#### Scenario: Several words in capitals are shortened

- **WHEN** the selections `SCREAMING_SNAKE_CASE` and `KEBAB-UPPERCASE` are each converted under `ABBR`
- **THEN** the results are `SSC` and `KU`

#### Scenario: A single word in capitals is kept

- **WHEN** the selections `ABBR` and `UPPERCASE` are each converted under `ABBR`
- **THEN** the results are `ABBR` and `UPPERCASE`, and the style is marked as having no effect

#### Scenario: An acronym in mixed text is kept

- **WHEN** the selection is `HTTPServer` under `ABBR`
- **THEN** the result is `HTTPS`

#### Scenario: Any other character separates words

- **WHEN** the selections `openspec/changes/add-abbreviation-case-style` and `/opsx:archive` are each converted under `ABBR`
- **THEN** the results are `OCAACS` and `OA`

#### Scenario: Numbers are kept whole

- **WHEN** the selections `Windows 11` and `All 14 of your examples` are each converted under `ABBR`
- **THEN** the results are `W11` and `A14OYE`
- **AND** with the digits option set the result for `Windows 11` is `W`

#### Scenario: Text outside the Latin alphabet is abbreviated

- **WHEN** the selection is `привіт світ → ПС` under `ABBR`
- **THEN** the result is `ПСПС`

#### Scenario: A selection spans several lines

- **WHEN** a selection of two lines, `snake case` and `dot case`, is converted under `ABBR` with no line option set
- **THEN** the result is `SC`, the original line break, and `DC`
- **AND** with `lines: join` set the result is `SCDC`

#### Scenario: Whitespace options cannot change the abbreviation

- **WHEN** `ABBR` is selected
- **THEN** the `spaces: remove` and `spaces: normalize` options are marked as having no effect
