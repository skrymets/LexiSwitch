## Purpose

The user-facing behavior of LexiSwitch: three explicitly triggered actions that convert
mistyped text in place, and the cycling rule that chooses which layout to convert into and
how to undo.

## ADDED Requirements

### Requirement: Correction happens only when requested

The system SHALL convert text only in response to an explicit hotkey press. The system
SHALL NOT alter text automatically, on any heuristic, at any confidence level.

#### Scenario: User types mistyped text and does nothing

- **WHEN** the user types text under the wrong layout and continues working
- **THEN** the text is left exactly as typed
- **AND** no correction, prompt, or indication occurs

### Requirement: Convert the last typed word

Pressing the last-word hotkey SHALL replace the most recently typed word with its
conversion, in place, in the focused application.

#### Scenario: Mistyped word is corrected

- **WHEN** the user types `ауфегкуі` and presses the last-word hotkey
- **THEN** the text is replaced with `features`
- **AND** the cursor is left where it would be had the corrected text been typed directly

#### Scenario: No recent typing is available

- **WHEN** the user presses the last-word hotkey and no recent typing was recorded
- **THEN** nothing is typed, deleted, or altered
- **AND** the system briefly indicates it had nothing to correct

#### Scenario: Conversion would change nothing

- **WHEN** the last word converts to itself
- **THEN** no characters are deleted and none are typed
- **AND** the document is left byte-identical

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

### Requirement: Convert the clipboard

Pressing the clipboard hotkey SHALL convert the text currently held in the clipboard and
write the result back to the clipboard.

#### Scenario: Clipboard text is converted

- **WHEN** the clipboard holds mistyped text and the user presses the clipboard hotkey
- **THEN** the clipboard afterward holds the converted text
- **AND** nothing is pasted and no application receives synthetic input

#### Scenario: Clipboard holds no text

- **WHEN** the clipboard holds no text content
- **THEN** the clipboard is left unchanged
- **AND** the system indicates there was nothing to convert

### Requirement: Repeated presses cycle through candidate layouts

Repeated presses of the same hotkey within the configured multi-tap window SHALL render
the original text under successive candidate layouts, returning to the original text after
the last candidate. Each press SHALL render from the retained original rather than from
the previously converted result.

#### Scenario: Two layouts installed

- **WHEN** the user presses the hotkey twice within the multi-tap window
- **THEN** the first press converts the text
- **AND** the second press restores the original text exactly

#### Scenario: Three layouts installed

- **WHEN** the user presses the hotkey three times within the multi-tap window
- **THEN** the first press renders the text under the second candidate layout
- **AND** the second press renders it under the third candidate
- **AND** the third press restores the original text

#### Scenario: Multi-tap window elapses

- **WHEN** the user presses the hotkey, waits longer than the multi-tap window, and
  presses again
- **THEN** the second press is treated as a new correction of the text now present
- **AND** not as a continuation of the previous cycle

#### Scenario: Text was deliberately typed in the other layout

- **WHEN** the user corrects a technical term or password that was intentionally typed
  under the other layout
- **THEN** pressing the hotkey again within the multi-tap window restores it exactly
- **AND** no residue of the intermediate conversion remains

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
