## ADDED Requirements

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
