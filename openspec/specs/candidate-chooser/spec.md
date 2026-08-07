# candidate-chooser Specification

## Purpose
The conversion transaction: what the user sees between invoking LexiSwitch and the selected
text either changing or not. It exists so that a conversion is something the user chooses
rather than something that happens to them.
## Requirements
### Requirement: One chooser for every invocation

Invoking the system with at least one candidate SHALL open a single chooser. The same chooser
SHALL be used regardless of how many candidates exist or how confident the ranking is.
Confidence SHALL affect only ordering and preselection.

The system SHALL NOT require repeated presses of the invocation shortcut to reveal further
candidates, and SHALL NOT present candidates as pages.

#### Scenario: One candidate exists

- **WHEN** the selected text yields exactly one candidate
- **THEN** the chooser opens with that candidate preselected
- **AND** the text is not modified until the user commits

#### Scenario: Several candidates exist

- **WHEN** the selected text yields more than one candidate
- **THEN** all of them are visible in one list
- **AND** no candidate is reachable only by invoking the shortcut again

### Requirement: The chooser shows what is being decided

The chooser SHALL display a preview of the original selected text, the source and target
layout of every candidate, a preview of every candidate's result, and the currently selected
candidate.

It SHALL display the current Windows layout as context, presented so that it cannot be
mistaken for the source layout of any candidate.

Where a candidate leaves characters unconverted, the chooser SHALL report how many.

#### Scenario: User reviews a candidate

- **WHEN** the chooser is open
- **THEN** the user can see the original text, each candidate's source and target layout, and what each candidate would produce

#### Scenario: A candidate converts only part of the selection

- **WHEN** a candidate leaves characters unchanged because they map to no key in its source layout
- **THEN** the chooser reports the number of unchanged characters for that candidate

### Requirement: The chooser is operable from the keyboard alone

Every action in the transaction SHALL be reachable from the keyboard. No required step SHALL
depend on the mouse.

Arrow keys SHALL move between candidates. Moving between candidates SHALL NOT modify the
selected text, and each candidate SHALL be rendered from the original selection rather than
from any previously selected candidate's result.

#### Scenario: User moves through candidates

- **WHEN** the user presses the down or up arrow
- **THEN** the selected candidate changes
- **AND** the document is unchanged

#### Scenario: User moves through candidates repeatedly

- **WHEN** the user moves back and forth between candidates several times
- **THEN** each candidate shows the same result it showed the first time

### Requirement: Long selections can be inspected without leaving the transaction

A candidate preview SHALL be compact enough that all candidates remain comparable in one list.
The system SHALL provide an expanded view of the selected candidate, and expanding SHALL NOT
end the transaction or modify the text.

Where the preview omits content, the omission SHALL be visible. A compact preview SHOULD
favour a region containing changed text rather than always beginning at the start of the
selection.

#### Scenario: Selection is longer than a row can show

- **WHEN** a candidate's result is too long for its row
- **THEN** the row shows a truncated preview and indicates that content was omitted

#### Scenario: User expands a preview

- **WHEN** the user expands the selected candidate
- **THEN** the fuller text is shown and can be scrolled from the keyboard
- **AND** the transaction remains open and the document unmodified

### Requirement: Applying is explicit

The system SHALL NOT modify the selected text before the user commits.

Committing without a layout change SHALL replace the selection with the selected candidate and
SHALL NOT intentionally change the active input language. Committing with a layout change SHALL
additionally request activation of that candidate's target layout, and the chooser SHALL name
that layout before the user commits.

After any commit the system SHALL restore focus to the application the text was selected in,
and the transaction SHALL close.

#### Scenario: User applies a candidate

- **WHEN** the user commits the selected candidate without requesting a layout change
- **THEN** the selection is replaced with that candidate's result
- **AND** the active input language is not intentionally changed
- **AND** focus returns to the original application

#### Scenario: User applies and switches layout

- **WHEN** the user commits with a layout change
- **THEN** the selection is replaced and activation of that candidate's target layout is requested
- **AND** the outcome distinguishes a successful switch from one that could not be confirmed

#### Scenario: User can see what the layout change will do

- **WHEN** the chooser is open
- **THEN** the target layout that a layout-changing commit would activate is named in the chooser

### Requirement: Cancelling changes nothing

Cancelling SHALL close the chooser, leave the selected text unchanged, restore focus to the
original application, restore any clipboard state the transaction disturbed, and discard the
transaction.

#### Scenario: User cancels

- **WHEN** the user cancels an open chooser
- **THEN** the selected text is unchanged and no candidate is applied
- **AND** the clipboard holds what it held before the transaction
- **AND** focus returns to the original application

### Requirement: Outcomes are distinguishable

The system SHALL distinguish success, cancellation, no valid conversion, failure to read the
selection, failure to replace, and failure to switch layout.

The system SHALL NOT report success when replacement has failed or cannot reasonably be
assumed to have succeeded. Where a replacement fails, the system SHALL preserve or restore the
prior state as far as it can.

Feedback about a transaction SHALL appear in the transaction. Conditions that persist beyond a
transaction SHALL be reflected where the user can find them without invoking a conversion.

#### Scenario: No candidate survives filtering

- **WHEN** every generated candidate is identical to the original or is a duplicate
- **THEN** the system reports that no distinct conversion was found
- **AND** this is distinguishable from a failure

#### Scenario: The selection cannot be read

- **WHEN** the focused application exposes no readable selection and the fallback also fails
- **THEN** the system reports that the selection could not be read
- **AND** no replacement is attempted

#### Scenario: Replacement fails

- **WHEN** a commit is made and the replacement does not succeed
- **THEN** the system does not report success
- **AND** the original text and clipboard state are restored as far as possible

### Requirement: Nothing from a transaction is retained

The system SHALL NOT write the original selected text, any candidate, or any preview to a log
or to any file, and SHALL NOT retain them after the transaction closes.

Where the clipboard is used to read or replace the selection, the system SHALL minimise how
long its content is present and SHALL restore the previous clipboard state where possible.

The system SHALL NOT claim that clipboard-derived text cannot enter Windows Clipboard History
unless that is structurally guaranteed rather than requested.

#### Scenario: Transaction closes

- **WHEN** a transaction ends for any reason
- **THEN** no original text, candidate, or preview remains held by the system

#### Scenario: Logging is enabled during a conversion

- **WHEN** logging is enabled and a conversion is performed
- **THEN** the records describe event kinds and sizes only, and contain no selected or converted text

