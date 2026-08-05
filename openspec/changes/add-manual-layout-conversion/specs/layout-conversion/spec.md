## Purpose

Converts text that was typed under the wrong keyboard layout into what the user intended,
by reinterpreting physical key positions under a different layout rather than translating
between alphabets.

## ADDED Requirements

### Requirement: Layout tables derived from installed layouts

The system SHALL build its keyboard layout tables at startup from the layouts the
operating system reports as installed. The system SHALL NOT contain compiled-in layout
data.

#### Scenario: User has Ukrainian, English, and German installed

- **WHEN** the system starts
- **THEN** it builds a table for each of the three installed layouts
- **AND** conversion between any pair of them is available without configuration

#### Scenario: User installs a fourth layout

- **WHEN** the user adds a keyboard layout in Windows settings and restarts the system
- **THEN** the new layout becomes available as a conversion candidate
- **AND** no code change, rebuild, or release is required

#### Scenario: Layout variant differs from the common case

- **WHEN** the installed English layout is US International rather than US
- **THEN** the derived table reflects US International
- **AND** conversions produce characters matching what that layout actually types

### Requirement: Minimum of two layouts

The system SHALL refuse to start when fewer than two keyboard layouts are installed, and
SHALL explain why rather than failing silently or running inert.

#### Scenario: Only one layout installed

- **WHEN** the system starts with a single installed layout
- **THEN** it does not start
- **AND** it reports that at least two layouts are required, and that layouts are added
  in Windows settings

### Requirement: Conversion is position-based

The system SHALL convert text by mapping each character to the physical key that produces
it under the source layout, then to the character that key produces under the target
layout. Conversion SHALL preserve the shift state, and therefore the case, of each
character.

#### Scenario: Ukrainian text typed under an English layout

- **WHEN** the text `ауфегкуі` is converted from Ukrainian to English
- **THEN** the result is `lexiswitch`

#### Scenario: Mixed case is preserved

- **WHEN** text containing both upper- and lower-case characters is converted
- **THEN** each character's case in the result matches the shift state it was typed with

#### Scenario: QWERTZ transposition

- **WHEN** text is converted between a German QWERTZ layout and a QWERTY layout
- **THEN** `y` and `z` are exchanged according to their physical positions, not treated as
  the same key

### Requirement: Unmappable characters pass through unchanged

The system SHALL pass through unchanged any character that maps to no key in the source
layout, or whose key produces a dead key in either layout. The system SHALL NOT substitute
an approximate or plausible-looking character.

#### Scenario: Character absent from the source layout

- **WHEN** converted text contains a character the source layout cannot produce
- **THEN** that character appears unchanged in the result
- **AND** the remaining characters convert normally

#### Scenario: Dead key position encountered

- **WHEN** a character maps to a position that is a dead key in either layout
- **THEN** that character appears unchanged in the result
- **AND** no combining or accented character is invented in its place

### Requirement: Conversion producing no change is reported as no change

The system SHALL distinguish a conversion that alters the text from one that does not, so
that callers can decline to act rather than replacing text with an identical copy.

#### Scenario: Text is identical under both layouts

- **WHEN** text consisting only of characters that map identically in both layouts is
  converted
- **THEN** the system reports that no change would occur
- **AND** no replacement is performed

### Requirement: Round-trip fidelity

Converting text from layout A to layout B and back to layout A SHALL return the original
text, for any text composed entirely of characters mappable in both layouts.

#### Scenario: Round trip over mappable text

- **WHEN** text composed only of characters present in both layouts is converted to the
  other layout and back
- **THEN** the result is identical to the original text
