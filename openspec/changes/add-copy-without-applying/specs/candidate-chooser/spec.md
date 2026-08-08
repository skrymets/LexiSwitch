## ADDED Requirements

### Requirement: A result can be copied without being applied

The chooser SHALL provide a way to place the selected candidate's result on the clipboard
without modifying the document.

Copying SHALL NOT close the chooser, SHALL NOT end the transaction, and SHALL NOT modify the
selected text. The user SHALL be able to copy, continue moving between candidates, and then
either commit or cancel.

What is copied SHALL be the candidate's whole result, not the shortened preview shown in its
row, and SHALL be the same text that committing that candidate would write.

The chooser SHALL confirm that a copy was made, and SHALL do so only when the clipboard was
actually written — a copy that failed SHALL NOT be reported as one that succeeded.

The chooser SHALL offer a way to choose whether the copied text is exposed to Windows Clipboard
History. It SHALL default to excluding it, and SHALL return to that default every time the
chooser opens, so that exposure is always something the user asked for in this transaction
rather than something left switched on from an earlier one.

Where the user changes that choice after copying, the system SHALL bring the clipboard into
line with the new choice, so that what is on the clipboard and what the chooser says about it
cannot disagree.

Copying SHALL remain available when the clipboard fallback for *reading* a selection is
switched off. That setting governs what the system does unbidden; an explicit copy is an
instruction.

#### Scenario: User copies a candidate and cancels

- **WHEN** the user copies the selected candidate and then cancels
- **THEN** the document is unchanged
- **AND** the clipboard holds that candidate's result

#### Scenario: User copies one candidate and applies another

- **WHEN** the user copies one candidate, moves to a different one, and commits it
- **THEN** the document holds the committed candidate's result
- **AND** the clipboard still holds the copied candidate's result

#### Scenario: The copy cannot be made

- **WHEN** the clipboard cannot be written
- **THEN** the chooser does not report a copy
- **AND** the transaction is otherwise unaffected

#### Scenario: User copies with the reading fallback switched off

- **WHEN** the clipboard fallback for reading a selection is disabled and the user copies a candidate
- **THEN** the copy is made

## MODIFIED Requirements

### Requirement: Cancelling changes nothing

Cancelling SHALL close the chooser, leave the selected text unchanged, restore focus to the
original application, restore any clipboard state the transaction disturbed, and discard the
transaction.

Where the user has deliberately copied a result during the transaction, that copy SHALL survive
cancellation. The clipboard state the system restores is the state it disturbed on the user's
behalf, not a copy the user asked for — restoring over it would undo the only thing the
transaction was invoked to produce.

#### Scenario: User cancels

- **WHEN** the user cancels an open chooser
- **THEN** the selected text is unchanged and no candidate is applied
- **AND** the clipboard holds what it held before the transaction
- **AND** focus returns to the original application

#### Scenario: User cancels after copying

- **WHEN** the user copies a result and then cancels
- **THEN** the clipboard holds the copied result rather than the contents from before the transaction
- **AND** the selected text is still unchanged

### Requirement: Nothing from a transaction is retained

The system SHALL NOT write the original selected text, any candidate, or any preview to a log
or to any file, and SHALL NOT retain them after the transaction closes.

Where the clipboard is used to read or replace the selection, the system SHALL minimise how
long its content is present and SHALL restore the previous clipboard state where possible.

This constrains what the *system* keeps of its own accord. Text the user has explicitly copied
is theirs and SHALL be left on the clipboard, and SHALL be the state restored if the transaction
later has to put the clipboard back. Where such a copy is marked to be kept out of Clipboard
History, every subsequent write of it SHALL carry the same marking, so that restoring it at the
end of the transaction does not expose what copying it did not.

The system SHALL NOT claim that clipboard-derived text cannot enter Windows Clipboard History
unless that is structurally guaranteed rather than requested.

#### Scenario: Transaction closes

- **WHEN** a transaction ends for any reason
- **THEN** no original text, candidate, or preview remains held by the system

#### Scenario: Logging is enabled during a conversion

- **WHEN** logging is enabled and a conversion is performed
- **THEN** the records describe event kinds and sizes only, and contain no selected or converted text

#### Scenario: A copy marked out of history is restored

- **WHEN** a copy marked to stay out of Clipboard History is put back on the clipboard as the transaction ends
- **THEN** it is written with the same marking it was copied with
