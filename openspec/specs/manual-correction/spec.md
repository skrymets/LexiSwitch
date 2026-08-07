# manual-correction Specification

## Purpose
The user-facing behavior of LexiSwitch: three explicitly triggered actions that convert
mistyped text in place, and the cycling rule that chooses which layout to convert into and
how to undo.
## Requirements
### Requirement: Correction happens only when requested

The system SHALL convert text only in response to an explicit hotkey press. The system
SHALL NOT alter text automatically, on any heuristic, at any confidence level.

#### Scenario: User types mistyped text and does nothing

- **WHEN** the user types text under the wrong layout and continues working
- **THEN** the text is left exactly as typed
- **AND** no correction, prompt, or indication occurs

### Requirement: Convert the selection

Pressing the selection hotkey SHALL replace the currently highlighted text with its
conversion, in place.

#### Scenario: Selected text is converted

- **WHEN** the user selects mistyped text and presses the selection hotkey
- **THEN** the selection is replaced with the converted text

#### Scenario: Application does not expose its selection

- **WHEN** the focused application provides no way to read the selection directly
- **THEN** the system falls back to a clipboard-based replacement
- **AND** the user's previous clipboard contents are restored afterward

#### Scenario: Clipboard is unavailable during the fallback

- **WHEN** the clipboard cannot be acquired after retries
- **THEN** the operation is abandoned without pasting anything
- **AND** the user's clipboard contents are left undamaged
- **AND** the failure is reported

#### Scenario: Clipboard fallback is disabled

- **WHEN** the user has disabled the clipboard fallback and the application does not
  expose its selection
- **THEN** no conversion occurs
- **AND** the system reports that the application is not supported without the fallback

### Requirement: Active layout follows a correction

After a successful correction the system SHALL switch the active keyboard layout to the
one the text was converted into, so the user can continue typing in that language. This
behavior SHALL be configurable and SHALL be enabled by default.

#### Scenario: Correction into Ukrainian

- **WHEN** text is corrected into the Ukrainian layout
- **THEN** the active layout for the focused window becomes Ukrainian
- **AND** subsequent typing produces Ukrainian characters without further action

#### Scenario: Layout following is disabled

- **WHEN** the user has disabled layout following and a correction occurs
- **THEN** the text is corrected
- **AND** the active layout is left unchanged

