# layout-conversion Specification

## Purpose
Converts text that was typed under the wrong keyboard layout into what the user intended,
by reinterpreting physical key positions under a different layout rather than translating
between alphabets.
## Requirements
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

Conversion requires at least two installed keyboard layouts, because a layout cannot be
converted to itself.

Where fewer than two are installed, the system SHALL start in a disabled state and SHALL
explain that at least two are required. It SHALL NOT exit, and SHALL NOT run in a state where
invoking it appears to do nothing.

Settings and diagnostics SHALL remain reachable in this state, so that a user who has just
been told what is required can act on it and does not have to reinstall or reconfigure blind.

The system SHALL become usable without a restart once a second layout is installed.

#### Scenario: Only one layout installed

- **WHEN** the system starts with fewer than two keyboard layouts installed
- **THEN** it starts in a disabled state that explains at least two layouts are required
- **AND** it does not exit
- **AND** settings and diagnostics remain reachable

#### Scenario: A second layout is installed while running

- **WHEN** the user installs a second layout while the system is running in the disabled state
- **THEN** the system becomes able to convert without being restarted

#### Scenario: Two or more layouts installed

- **WHEN** the system starts with two or more keyboard layouts installed
- **THEN** it builds conversion tables and enters the enabled state

### Requirement: Conversion is position-based

The system SHALL convert text by mapping each character to the physical key that produces
it under the source layout, then to the character that key produces under the target
layout. Conversion SHALL preserve the shift state, and therefore the case, of each
character.

#### Scenario: Ukrainian text typed under an English layout

- **WHEN** the text `ауфегкуі` is converted from Ukrainian to English
- **THEN** the result is `features`

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

### Requirement: Candidates are generated from the selected text, not from the active layout

The system SHALL determine which conversions are plausible from the characters in the selected
text and the set of installed layouts. The input language currently active in the foreground
window SHALL NOT be treated as evidence of which layout the text was originally typed under.

The active layout MAY be displayed as context and MAY contribute as a weak ranking signal. It
SHALL NOT determine the source layout of any candidate.

#### Scenario: Text was typed under a layout that is no longer active

- **WHEN** the user selects text typed under one layout while a different layout is now active
- **THEN** candidates are still generated for the layout that could have produced the characters
- **AND** the conversion does not depend on the user restoring the earlier layout first

#### Scenario: The active layout cannot produce the selected characters

- **WHEN** the selected characters map to no key in the currently active layout
- **THEN** this does not prevent candidate generation

### Requirement: Every non-self pair of installed layouts is considered

For every ordered pair of installed layouts whose source and target differ, the system SHALL
produce a candidate by reinterpreting the selected text's key positions from source to target.
Pairs whose source and target are the same layout SHALL NOT be produced.

#### Scenario: Three layouts are installed

- **WHEN** three layouts are installed and text is selected
- **THEN** the system considers each of the six ordered source-to-target pairs
- **AND** no pair converting a layout to itself is considered

### Requirement: Candidates that change nothing are discarded

The system SHALL discard any candidate whose result is identical to the original selected
text.

#### Scenario: A source layout cannot represent the selected characters

- **WHEN** a candidate's source layout produces none of the characters in the selection
- **THEN** every character passes through unchanged, the result equals the original, and the candidate is discarded

#### Scenario: No candidate survives

- **WHEN** every generated candidate is identical to the original
- **THEN** the system reports that no distinct conversion was found rather than presenting an empty list as a failure

### Requirement: Candidates are deduplicated by result and target together

Two candidates SHALL be treated as the same only when both their visible result and their
target layout are the same. Candidates producing identical text for different target layouts
SHALL both be retained.

This is because the target layout determines what a layout-changing commit activates.
Collapsing such candidates on visible text alone would silently remove a choice the user is
being asked to make.

#### Scenario: Two pairs produce identical text with different targets

- **WHEN** two candidates produce the same visible result but name different target layouts
- **THEN** both are retained and both are shown
- **AND** each is labelled with its own source and target so they can be told apart

#### Scenario: Two pairs produce identical text with the same target

- **WHEN** two candidates produce the same visible result and name the same target layout
- **THEN** only one is retained

### Requirement: Ranking is deterministic and self-contained

The system SHALL order candidates using signals derived only from the selected text, the
installed layout tables, and the system's own state.

Ranking SHALL NOT require a dictionary, a language model, network access, downloaded data, or
any file shipped alongside the executable.

The system SHALL NOT present a candidate as linguistically certain. Where a source or result is
mechanically unique it MAY be described as detected; where ordering results from heuristics it
SHALL be described as suggested.

#### Scenario: The source character set identifies one layout

- **WHEN** the selected characters can be produced by exactly one installed layout
- **THEN** candidates from that source rank above candidates from sources that cannot produce those characters

#### Scenario: The source is ambiguous between layouts

- **WHEN** the selected characters can be produced by more than one installed layout
- **THEN** the system ranks by deterministic signals and does not claim to know which layout was used

