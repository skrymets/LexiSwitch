# case-conversion Specification

## Purpose
Changing the case of selected text — and optionally stripping punctuation, digits or
whitespace from it — as a transaction the user invokes deliberately, sees the result of
before anything is modified, and commits or abandons. It is a sibling of layout conversion,
not a variant of it.
## Requirements
### Requirement: Case conversion is invoked separately from layout conversion

Case conversion SHALL have its own invocation shortcut, distinct from the layout-conversion
shortcut, and SHALL present its candidates in a window of its own.

Case styles SHALL NOT appear among layout-conversion candidates, and layout-conversion
candidates SHALL NOT appear among case styles. Neither invocation SHALL offer the other's
results, and neither SHALL require the user to navigate past the other's results.

The default binding SHALL be one that can actually be registered on a stock Windows 11
installation. A default that cannot be registered SHALL be treated as a defect, not as a
conflict for the user to resolve.

#### Scenario: User invokes case conversion

- **WHEN** the user selects text and presses the case-conversion shortcut
- **THEN** a case window opens showing only case styles
- **AND** no keyboard-layout candidate is present

#### Scenario: User invokes layout conversion

- **WHEN** the user selects text and presses the layout-conversion shortcut
- **THEN** the existing chooser opens showing only layout candidates
- **AND** no case style is present

### Requirement: The offered styles are a fixed, grouped list

The system SHALL offer exactly these styles, in exactly this order, in three groups:

1. `As typed`, `Invert case`, `lowercase`, `UPPERCASE`
2. `camelCase`, `snake_case`, `dot.case`, `kebab-lowercase`, `PascalCase`,
   `SCREAMING_SNAKE_CASE`, `Capitalized_Snake_Case`, `KEBAB-UPPERCASE`
3. `First word capitalized`, `Words Capitalized`

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

### Requirement: Strip options modify every offered result

The system SHALL offer six strip options: **punctuation**, **digits**, and two mutually
exclusive pairs — spaces **remove** or **normalize**, and line breaks **join** or **remove**.

Within each pair, setting one member SHALL clear the other. The two members of a pair SHALL
NOT both be set.

Setting an option SHALL immediately update the previewed result of **every** style, not only
the selected one, so that styles remain comparable under the options in force.

All options SHALL begin unset on every invocation. The system SHALL NOT carry a strip option
from one invocation into the next, so that a destructive option cannot be silently in force.

- **punctuation** removes every character that is neither a letter nor a digit from within
  words, leaving separators alone.
- **digits** removes digit characters from within words.
- **spaces: remove** deletes horizontal whitespace separators; **spaces: normalize** replaces
  each run of horizontal whitespace with a single space and converts tabs to spaces. Neither
  trims the ends of the selection.
- **lines: join** replaces each line break with a single space; **lines: remove** deletes it.

#### Scenario: User sets one member of an exclusive pair while the other is set

- **WHEN** the user sets `spaces: normalize` while `spaces: remove` is already set
- **THEN** `spaces: remove` is cleared
- **AND** only `spaces: normalize` is in force

#### Scenario: User sets an option

- **WHEN** the user sets the punctuation option
- **THEN** every style's previewed result updates to show punctuation removed

#### Scenario: User invokes the window again after using an option

- **WHEN** the user commits with the punctuation option set, then invokes case conversion again
- **THEN** the punctuation option is unset

### Requirement: Transforms are defined on words, not on characters

A transform SHALL be defined as: split the selection into words and separators; remove
stripped characters from words; resolve line-break separators; resolve horizontal-whitespace
separators; apply the style; join.

A word boundary SHALL survive the removal of the separator character that produced it, so
that a style which capitalises word beginnings still does so after whitespace has been
removed.

Line breaks SHALL be resolved before horizontal whitespace, so that a break joined into a
space is then governed by the horizontal-whitespace option.

Word boundaries SHALL be found at whitespace, at `_`, `-` and `.`, and at a lowercase letter
followed by an uppercase letter. A boundary SHALL NOT be placed between a letter and an
adjacent digit.

The eight styles in group 2 above produce identifiers, and SHALL therefore drop every
character that is neither a letter nor a digit regardless of the strip options, since the
delimiter they join with is the style's own.

#### Scenario: Whitespace is removed under a capitalising style

- **WHEN** the selection is `Hello World` under `Words Capitalized` with `spaces: remove` set
- **THEN** the result is `HelloWorld`

#### Scenario: Lines are joined and whitespace normalized together

- **WHEN** a selection whose two lines are separated by a break and trailing spaces is transformed with `lines: join` and `spaces: normalize`
- **THEN** exactly one space separates the last word of the first line from the first word of the second

#### Scenario: A word contains a digit

- **WHEN** the selection is `getUserV2` under `snake_case`
- **THEN** the result is `get_user_v2` and not `get_user_v_2`

#### Scenario: Prose is converted to an identifier style

- **WHEN** a selection containing sentence punctuation is converted under `camelCase`
- **THEN** the punctuation is absent from the result
- **AND** the result is shown before it is applied

### Requirement: Nothing is filtered out; ineffective choices are marked

The system SHALL present every style and every strip option at all times. It SHALL NOT remove
or hide one because it would not change the selected text.

A style or option that would not change the result, given the current selection and the
options in force, SHALL be visibly marked as having no effect.

Committing a style that would not change the text SHALL leave the document untouched and end
the transaction, and SHALL NOT perform a replacement with identical text.

#### Scenario: A style would change nothing

- **WHEN** the selection is already entirely uppercase and `UPPERCASE` is offered
- **THEN** `UPPERCASE` is shown, marked as having no effect
- **AND** it is still selectable

#### Scenario: An option would change nothing

- **WHEN** the selection contains no digits
- **THEN** the digits option is shown, marked as having no effect

#### Scenario: User commits a style that would change nothing

- **WHEN** the user commits a style marked as having no effect
- **THEN** the document is unchanged
- **AND** the transaction ends without reporting a failure

### Requirement: Frequently used styles are promoted without moving

The system SHALL show the three most frequently applied styles above the full list, ordered by
how often each has been applied.

The promoted entries SHALL be a **copy**: the full list below SHALL retain every style in its
declared position, so promotion never moves a style out from under the user.

The promoted order SHALL NOT change while the window is open. It SHALL be recalculated only
between invocations.

The system SHALL provide a way to discard the accumulated counts from within the window,
without ending the transaction, after which the promoted entries revert to the declared order
and counting begins again.

The counts SHALL survive a restart of the system.

#### Scenario: A style is applied repeatedly

- **WHEN** the user has applied one style more often than any other
- **THEN** it appears first among the promoted entries on the next invocation

#### Scenario: A style is promoted

- **WHEN** a style appears among the promoted entries
- **THEN** it also still appears at its declared position in the full list

#### Scenario: A style is applied while the window is open

- **WHEN** the user is looking at the window
- **THEN** the promoted entries do not reorder until the window is next opened

#### Scenario: User discards the counts

- **WHEN** the user discards the accumulated counts from within the window
- **THEN** the promoted entries revert to the declared order
- **AND** the window remains open and the document unmodified

#### Scenario: The system restarts

- **WHEN** the system is restarted
- **THEN** the promoted entries reflect the counts accumulated before the restart

### Requirement: The case window is operable from the keyboard alone

Every action SHALL be reachable from the keyboard. No required step SHALL depend on the mouse.

Arrow keys SHALL move between styles. Each strip option SHALL be reachable by a single key
press that does not require leaving the selected style. Moving between styles and setting
options SHALL NOT modify the selected text.

The system SHALL provide an expanded view of the selected style's result for selections too
long to show in a row, scrollable from the keyboard, and expanding SHALL NOT end the
transaction or modify the text. Where a preview omits content, the omission SHALL be visible.

The window SHALL name its own key bindings.

#### Scenario: User moves through styles

- **WHEN** the user presses the down or up arrow
- **THEN** the selected style changes and the document is unchanged

#### Scenario: User sets an option without losing their place

- **WHEN** the user presses the key for a strip option
- **THEN** the option changes and the selected style is still selected

#### Scenario: Selection is longer than a row can show

- **WHEN** a style's result is too long for its row
- **THEN** the row shows a truncated preview and indicates that content was omitted

### Requirement: Applying is explicit and changes no keyboard layout

The system SHALL NOT modify the selected text before the user commits.

Committing SHALL replace the selection with the selected style's result. The system SHALL NOT
change the active input language as part of a case conversion, and the window SHALL NOT offer
to.

Cancelling SHALL close the window, leave the selected text unchanged, restore any clipboard
state the transaction disturbed, and discard the transaction.

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

### Requirement: Case-transaction outcomes are distinguishable

The system SHALL distinguish success, cancellation, failure to read the selection, and failure
to replace. It SHALL NOT report success when a replacement has failed or cannot reasonably be
assumed to have succeeded, and where a replacement fails it SHALL preserve or restore the
prior state as far as it can.

Feedback about a transaction SHALL appear in the transaction.

#### Scenario: The selection cannot be read

- **WHEN** the focused application exposes no readable selection and the fallback also fails
- **THEN** the system reports that the selection could not be read
- **AND** no replacement is attempted

#### Scenario: Replacement fails

- **WHEN** a commit is made and the replacement does not succeed
- **THEN** the system does not report success
- **AND** the original text and clipboard state are restored as far as possible

### Requirement: Nothing from a case transaction is retained

The system SHALL NOT write the original selected text, any transformed result, or any preview
to a log or to any file, and SHALL NOT retain them after the transaction closes.

The counts that drive promotion SHALL consist only of the system's own style identifiers and
integers. They SHALL NOT include selected text, any result, any application or window name, or
any timestamp.

#### Scenario: A case transaction closes

- **WHEN** a case transaction ends for any reason
- **THEN** no original text, result, or preview remains held by the system

#### Scenario: The stored counts are inspected

- **WHEN** the user opens the file holding the counts
- **THEN** it contains only style identifiers and integers
- **AND** no text the user typed or selected appears in it

