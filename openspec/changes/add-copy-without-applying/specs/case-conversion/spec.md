## ADDED Requirements

### Requirement: A result can be copied without being applied

The case window SHALL provide a way to place the selected style's result on the clipboard
without modifying the document.

Copying SHALL NOT close the window, SHALL NOT end the transaction, and SHALL NOT modify the
selected text. The user SHALL be able to copy, continue moving between styles or changing the
strip options, and then either commit or cancel.

What is copied SHALL be the style's whole result under the options currently in force, not the
shortened preview shown in its row, and SHALL be the same text that committing that style would
write. Copying from the expanded view SHALL copy the same text as copying from the list.

The window SHALL confirm that a copy was made, and SHALL do so only when the clipboard was
actually written.

The window SHALL offer a way to choose whether the copied text is exposed to Windows Clipboard
History. It SHALL default to excluding it, and SHALL return to that default every time the
window opens — the same rule the strip options follow, and for the same reason: an option that
changes what leaves the machine must not be inheritable from a transaction the user has
forgotten.

Where the user changes that choice after copying, the system SHALL bring the clipboard into
line with the new choice.

That choice SHALL be presented apart from the strip options. The strip options change the
result; this one changes only what happens to a copy of it, and presenting them together would
imply it alters the text.

Copying SHALL remain available when the clipboard fallback for *reading* a selection is switched
off.

#### Scenario: User copies a style and cancels

- **WHEN** the user copies the selected style and then cancels
- **THEN** the document is unchanged
- **AND** the clipboard holds that style's result

#### Scenario: User copies one style and applies another

- **WHEN** the user copies one style, moves to a different one, and commits it
- **THEN** the document holds the committed style's result
- **AND** the clipboard still holds the copied style's result

#### Scenario: Strip options are in force when copying

- **WHEN** the user sets a strip option and then copies the selected style
- **THEN** the copied text is the result with that option applied

#### Scenario: User changes the history choice after copying

- **WHEN** the user copies a result and then changes whether it may enter Clipboard History
- **THEN** the clipboard is brought into line with the new choice

## MODIFIED Requirements

### Requirement: Applying is explicit and changes no keyboard layout

The system SHALL NOT modify the selected text before the user commits.

Committing SHALL replace the selection with the selected style's result. The system SHALL NOT
change the active input language as part of a case conversion, and the window SHALL NOT offer
to.

Cancelling SHALL close the window, leave the selected text unchanged, restore any clipboard
state the transaction disturbed, and discard the transaction.

Where the user has deliberately copied a result during the transaction, that copy SHALL survive
both cancellation and commitment. The clipboard state the system restores is the state it
disturbed on the user's behalf, not a copy the user asked for.

After a commit or a cancellation the system SHALL restore focus to the application the text
was selected in.

#### Scenario: User applies a style

- **WHEN** the user commits the selected style
- **THEN** the selection is replaced with that style's result
- **AND** the active input language is unchanged
- **AND** focus returns to the original application

#### Scenario: User cancels

- **WHEN** the user cancels an open case window
- **THEN** the selected text is unchanged and no style is applied
- **AND** the clipboard holds what it held before the transaction
- **AND** focus returns to the original application

#### Scenario: User cancels after copying

- **WHEN** the user copies a result and then cancels
- **THEN** the clipboard holds the copied result rather than the contents from before the transaction
- **AND** the selected text is still unchanged

### Requirement: Nothing from a case transaction is retained

The system SHALL NOT write the original selected text, any transformed result, or any preview
to a log or to any file, and SHALL NOT retain them after the transaction closes.

This constrains what the *system* keeps of its own accord. Text the user has explicitly copied
is theirs and SHALL be left on the clipboard, and SHALL be the state restored if the transaction
later has to put the clipboard back. Where such a copy is marked to be kept out of Clipboard
History, every subsequent write of it SHALL carry the same marking.

The counts that drive promotion SHALL consist only of the system's own style identifiers and
integers. They SHALL NOT include selected text, any result, any application or window name, or
any timestamp. Copying a result SHALL NOT be counted as using the style — the counts record what
was applied to a document, and a copy is not that.

#### Scenario: A case transaction closes

- **WHEN** a case transaction ends for any reason
- **THEN** no original text, result, or preview remains held by the system

#### Scenario: The stored counts are inspected

- **WHEN** the user opens the file holding the counts
- **THEN** it contains only style identifiers and integers
- **AND** no text the user typed or selected appears in it

#### Scenario: A style is copied but never applied

- **WHEN** the user copies a style's result and cancels without applying anything
- **THEN** no style's count is incremented
