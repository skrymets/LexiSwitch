## Purpose

Rewriting selected Cyrillic text in Latin letters under a named scheme — an official one built
in, or one the user writes — as a transaction the user invokes deliberately, previews, and
commits or abandons. It is a sibling of layout conversion and case conversion, not a variant
of either.

## ADDED Requirements

### Requirement: Transliteration is invoked separately

Transliteration SHALL have its own invocation shortcut, distinct from the layout-conversion and
case-conversion shortcuts, and SHALL present its choices in a window of its own.

Transliterations SHALL NOT appear among layout candidates or case styles, and neither of those
SHALL appear among transliterations.

The default binding SHALL be `Win+Alt+Pause`, a combination that can be registered on a stock
Windows 11 installation.

#### Scenario: User invokes transliteration

- **WHEN** the user selects text and presses the transliteration shortcut
- **THEN** a transliteration window opens showing only rulesets
- **AND** no layout candidate and no case style is present

#### Scenario: Other invocations are unchanged

- **WHEN** the user presses the layout-conversion or case-conversion shortcut
- **THEN** the window that opens contains no transliteration

### Requirement: Every ruleset is offered every time

The window SHALL list every loaded ruleset, one row each, on every invocation. A ruleset whose
result would equal the selected text SHALL be shown marked as having no effect, not omitted.

Each row SHALL show the ruleset's name, its source and target language tags, and a preview of
its result. Where the result leaves Cyrillic letters untransliterated, the row SHALL say how
many.

Built-in rulesets SHALL come first, Ukrainian then russian, followed by user rulesets ordered by
name. A user ruleset that replaces a built-in SHALL take the built-in's position.

The first row that has an effect SHALL be preselected; if none has, the first row SHALL be.

#### Scenario: Selection contains no Cyrillic

- **WHEN** the user invokes transliteration on `Hello, world`
- **THEN** every ruleset is listed and every row is marked as having no effect

#### Scenario: A ruleset leaves letters untouched

- **WHEN** the selection contains Cyrillic letters the selected ruleset does not map
- **THEN** that row states how many letters were left untransliterated

### Requirement: The longest matching sequence wins

A ruleset SHALL map Cyrillic sequences of one or more characters to Latin text. At each
position the system SHALL apply the longest sequence that matches there, regardless of the
order in which the ruleset lists its entries.

A character no sequence matches SHALL be copied unchanged, except as the fallback requirement
below provides.

#### Scenario: A combination outranks its parts

- **WHEN** a ruleset maps `з` to `z`, `г` to `h` and `зг` to `zgh`, and the text is `розгон`
- **THEN** the result is `rozghon`

#### Scenario: Text outside the ruleset passes through

- **WHEN** the text is `Київ 2024, Kyiv!` under the Ukrainian ruleset
- **THEN** the result is `Kyiv 2024, Kyiv!`

### Requirement: Word-initial values apply at the start of a word

A ruleset MAY give a sequence a separate value for when it begins a word. That value SHALL be
used when the sequence begins a word, and the ordinary value everywhere else.

A position SHALL begin a word when the nearest preceding character that the ruleset does not
drop is not a letter, or when there is no preceding character. A dropped character therefore
does not split a word; a hyphen, space, digit or punctuation mark does.

#### Scenario: Word start and word middle

- **WHEN** the text is `Яготин` and then `Костянтин` under the Ukrainian ruleset
- **THEN** the results are `Yahotyn` and `Kostiantyn`

#### Scenario: An apostrophe does not start a word

- **WHEN** the text is `Знам'янка` under the Ukrainian ruleset
- **THEN** the result is `Znamianka`

#### Scenario: A hyphen starts a word

- **WHEN** the text is `Івано-Франківськ` under the Ukrainian ruleset
- **THEN** the result is `Ivano-Frankivsk`

### Requirement: Dropped characters and the fallback

A sequence mapped to empty text SHALL be removed from the result.

A ruleset MAY declare a fallback. Where it does, each Cyrillic letter that no sequence matches
SHALL be replaced by the fallback. Where it does not, such letters SHALL be copied unchanged.
The fallback SHALL NOT apply to any character outside the Cyrillic script.

#### Scenario: Soft sign and apostrophe are dropped

- **WHEN** the text is `Короп'є` under the Ukrainian ruleset
- **THEN** the result is `Koropie`

#### Scenario: A letter the ruleset does not cover

- **WHEN** a ruleset with fallback `?` and no entry for `ў` transliterates `ўі`
- **THEN** the result is `?i`
- **AND** the same ruleset without a fallback gives `ўi`

### Requirement: Capitalisation is derived from the source

A ruleset SHALL be written in lowercase only. The system SHALL derive the case of each result
from the text being transliterated:

- a lowercase match SHALL produce its value as written;
- an uppercase match that is part of an all-capitals run — the match contains two or more
  letters, all capitals, or the adjacent letter in the same word is a capital — SHALL produce
  its value in capitals;
- any other uppercase match SHALL produce its value with only the first letter capitalised.

#### Scenario: A capitalised word

- **WHEN** the text is `Щедрий` under the Ukrainian ruleset
- **THEN** the result is `Shchedryi`

#### Scenario: A word in capitals

- **WHEN** the text is `ЩУКА` under the Ukrainian ruleset
- **THEN** the result is `SHCHUKA`

#### Scenario: A single capital letter

- **WHEN** the text is `Я` under the Ukrainian ruleset
- **THEN** the result is `Ya`

### Requirement: Two rulesets are built in

The system SHALL include, with no setup:

- **Ukrainian**, following Cabinet of Ministers of Ukraine Resolution No. 55 of 27 January 2010,
  including its word-initial values for `є ї й ю я`, `зг` as `zgh`, and the soft sign and
  apostrophe dropped;
- **russian**, following ICAO Doc 9303.

Each built-in SHALL reproduce every example given in its source document.

#### Scenario: The Ukrainian built-in matches the resolution

- **WHEN** each example from the resolution's table is transliterated
- **THEN** every result equals the resolution's spelling

#### Scenario: The russian built-in matches ICAO

- **WHEN** the text is `Щукин Подъячев` under the russian ruleset
- **THEN** the result is `Shchukin Podieiachev`

### Requirement: Users add rulesets without a restart

The system SHALL load every ruleset file in a folder named `transliteration` beside the
configuration file. A user ruleset whose identifier equals a built-in's SHALL replace that
built-in. Adding, changing or removing a file SHALL take effect within a few seconds, without
a restart.

The system SHALL NOT write, create or modify any ruleset file. A missing folder SHALL mean that
only the built-ins are offered, and SHALL NOT be reported as a problem.

#### Scenario: User adds a ruleset

- **WHEN** the user saves a valid ruleset file into the folder while the system runs
- **THEN** within a few seconds the next transliteration window lists it

#### Scenario: User overrides a built-in

- **WHEN** the user saves a ruleset whose identifier is the built-in Ukrainian one's
- **THEN** the window lists the user's ruleset in the built-in's place, and not the built-in

### Requirement: A bad ruleset is reported and contained

A ruleset file SHALL be rejected whole when it does not parse, has an identifier that is not
lowercase letters, digits and hyphens, shares its identifier with another user file, has a
source or target that is not a well-formed language tag, has an empty sequence or a sequence
containing a capital letter, or contains a key or table the format does not define.

A rejected file SHALL be reported with its file name and the location of the problem in it.
The last version of that file that was valid, if any, SHALL stay in use. Every other ruleset
SHALL be unaffected, and the system SHALL keep running.

#### Scenario: User saves a broken ruleset

- **WHEN** a ruleset file is saved with an error
- **THEN** the system reports the file and the line of the error
- **AND** every other ruleset remains offered

#### Scenario: A previously valid ruleset breaks

- **WHEN** a ruleset that was loaded is saved again with an error
- **THEN** its previous valid version stays in use until the error is fixed

#### Scenario: A misspelled table

- **WHEN** a ruleset contains a table named `wordstart`
- **THEN** the file is rejected and the report names the unknown table

### Requirement: The transliteration window is operable from the keyboard alone

Every action SHALL be reachable from the keyboard. Arrow keys SHALL move between rulesets
without modifying the selected text. The system SHALL provide an expanded, scrollable view of
the selected result that does not end the transaction. The window SHALL name its own key
bindings.

The selected result SHALL be copyable to the clipboard without applying it, under the same
rules as the other two windows.

#### Scenario: User moves through rulesets

- **WHEN** the user presses the down or up arrow
- **THEN** the selected ruleset changes and the document is unchanged

#### Scenario: User copies without applying

- **WHEN** the user copies the selected result and then cancels
- **THEN** the clipboard holds the transliterated text and the document is unchanged

### Requirement: Applying is explicit and changes no keyboard layout

The system SHALL NOT modify the selected text before the user commits. Committing SHALL replace
the selection with the selected ruleset's result and SHALL NOT change the active input
language; the window SHALL NOT offer to.

Cancelling SHALL leave the text unchanged and restore any clipboard state the transaction
disturbed. After a commit or a cancellation, focus SHALL return to the application the text was
selected in.

The system SHALL distinguish success, cancellation, failure to read the selection and failure
to replace, and SHALL report them in the transaction.

#### Scenario: User applies a ruleset

- **WHEN** the user commits the selected ruleset
- **THEN** the selection is replaced with its result
- **AND** the active input language is unchanged
- **AND** focus returns to the original application

#### Scenario: User cancels

- **WHEN** the user cancels the window
- **THEN** the selected text and the clipboard are as they were before

### Requirement: Nothing from a transliteration transaction is retained

The system SHALL NOT write the selected text, any result or any preview to a log or to any
file, and SHALL NOT retain them after the transaction closes. Rulesets are compiled in memory
when loaded; the system SHALL NOT write a compiled form or any other derived file.

#### Scenario: A transliteration transaction closes

- **WHEN** a transliteration transaction ends for any reason
- **THEN** no original text, result or preview remains held by the system
- **AND** no file has been written
