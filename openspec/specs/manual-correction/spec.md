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

Pressing the invocation hotkey SHALL open a chooser over the currently highlighted text and
SHALL replace that text only after the user commits a candidate.

The selection SHALL be read while the user's application still has focus, before the chooser
opens.

#### Scenario: Selected text is converted

- **WHEN** the user selects mistyped text, presses the invocation hotkey, and commits a candidate
- **THEN** the selection is replaced with that candidate's result

#### Scenario: User invokes and changes their mind

- **WHEN** the user presses the invocation hotkey and then cancels
- **THEN** the selection is unchanged

#### Scenario: Application does not expose its selection

- **WHEN** the focused application provides no way to read the selection directly
- **THEN** the system falls back to a clipboard-based read
- **AND** the user's previous clipboard contents are restored afterward

#### Scenario: Clipboard is unavailable during the fallback

- **WHEN** the clipboard cannot be acquired after retries
- **THEN** the operation is abandoned without pasting anything
- **AND** the user's clipboard contents are left undamaged
- **AND** the failure is reported

#### Scenario: Clipboard fallback is disabled

- **WHEN** the user has disabled the clipboard fallback and the application does not expose its selection
- **THEN** no conversion occurs
- **AND** the system reports that the application is not supported without the fallback

### Requirement: Active layout follows a correction

The system SHALL change the active keyboard layout after a correction only when the user
commits with an explicit request to do so. A commit without that request SHALL NOT
intentionally change the active layout.

Where the request is made, the system SHALL name the target layout before the commit, and
SHALL report a switch that failed or could not be confirmed rather than assuming it succeeded.

#### Scenario: Correction into Ukrainian with an explicit switch

- **WHEN** the user commits a candidate whose target is Ukrainian, requesting a layout change
- **THEN** the text is replaced and the active layout for the focused window becomes Ukrainian

#### Scenario: Correction without a layout change

- **WHEN** the user commits a candidate without requesting a layout change
- **THEN** the text is replaced and the active layout is left unchanged

#### Scenario: The application declines the layout change

- **WHEN** a layout change is requested and the target application does not adopt it
- **THEN** the system reports that the switch could not be confirmed
- **AND** does not report the switch as successful

