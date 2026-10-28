## MODIFIED Requirements

### Requirement: Tray controls

The tray icon SHALL provide, at minimum: enable and disable, a command that invokes a
conversion, a command that invokes a case conversion, a command that invokes a
transliteration, settings, reload configuration, open the configuration file, open the
transliteration folder, diagnostics, and quit.

Disabling SHALL mean that the invocation hotkeys stop converting. It SHALL NOT be described as
pausing capture, because nothing is captured.

The tray SHALL distinguish at least these states, and SHALL show the active shortcuts where
they are registered: enabled, disabled, fewer than two layouts installed, shortcut conflict,
a ruleset that was rejected, and runtime error.

Every invocation command SHALL work whether or not the corresponding global shortcut is
registered, so that a shortcut conflict never leaves the product entirely unusable.

Opening the transliteration folder SHALL create the folder if it does not exist, and SHALL NOT
create any file in it.

#### Scenario: User disables from the tray

- **WHEN** the user selects disable
- **THEN** every hotkey stops converting
- **AND** the tray indicates the disabled state

#### Scenario: User invokes a conversion from the tray

- **WHEN** the user selects the conversion command with text selected in another application
- **THEN** a conversion transaction begins as though the shortcut had been pressed

#### Scenario: User invokes a case conversion from the tray

- **WHEN** the user selects the case-conversion command with text selected in another application
- **THEN** a case transaction begins as though the case shortcut had been pressed

#### Scenario: User invokes a transliteration from the tray

- **WHEN** the user selects the transliteration command with text selected in another application
- **THEN** a transliteration transaction begins as though the transliteration shortcut had been pressed

#### Scenario: User opens the transliteration folder

- **WHEN** the user selects the command to open the transliteration folder and it does not exist
- **THEN** the folder is created empty and shown

#### Scenario: The shortcut is unavailable

- **WHEN** a configured shortcut cannot be registered
- **THEN** the tray shows the conflict state and names the combination
- **AND** every invocation command remains usable

#### Scenario: User quits

- **WHEN** the user selects quit
- **THEN** hotkey registrations are released and the process exits
- **AND** no background process remains

### Requirement: Configuration is a readable file

Configuration SHALL be stored as a single human-readable text file that the user edits
directly. The file SHALL be the complete description of the system's **configurable
behavior**; the system SHALL NOT keep configurable state elsewhere, with one exception:
transliteration rulesets, which are user-authored, human-readable files in their own folder
beside the configuration file, one ruleset per file.

The system MAY keep non-configurable state — records of what the user has done, which change
as a side effect of ordinary use rather than as a decision — in a separate file beside the
configuration file. Such a file SHALL NOT hold any setting, SHALL NOT alter behavior the
configuration file describes, and its absence or corruption SHALL NOT prevent the system from
starting or be reported as an error.

A configuration file written by an earlier version SHALL NOT prevent the system from starting.
Settings that no longer exist SHALL be ignored, and the system SHALL report which ones it
ignored, so that a removed setting is distinguishable from a misspelled one.

Settings the user is expected to change during normal use — the invocation shortcuts and
whether the system is enabled — SHALL also be reachable from the interface, without requiring
the user to find and edit the file.

#### Scenario: User inspects configuration

- **WHEN** the user opens the configuration file
- **THEN** every configurable behavior other than the rulesets is visible in that one file
- **AND** the file can be copied, diffed, and version-controlled

#### Scenario: Configuration file predates this version

- **WHEN** the system loads a configuration file containing settings that have been removed
- **THEN** the system starts, applying the settings it still recognises
- **AND** it reports each ignored setting by name

#### Scenario: User changes the shortcut without editing the file

- **WHEN** the user records a new shortcut through the interface
- **THEN** the change takes effect and is persisted to the configuration file

#### Scenario: The state file is missing or unreadable

- **WHEN** the system starts and the separate state file is absent, unreadable or malformed
- **THEN** the system starts normally with no accumulated state
- **AND** it does not report an error

## ADDED Requirements

### Requirement: A default shortcut never invalidates a configuration

Two invocation shortcuts SHALL NOT be bound to the same combination. Where the user has written
both bindings, the configuration SHALL be rejected as before.

Where one of the colliding bindings is a default — its key is absent from the file — and the
other was written by the user, the user's binding SHALL win. The defaulted invocation SHALL be
left without a shortcut, the system SHALL start, and it SHALL report which invocation has no
shortcut and why, until the user binds one. The invocation SHALL remain usable from the tray.

#### Scenario: A new default collides with an existing binding

- **WHEN** the configuration sets `change_case = "Win+Alt+Pause"` and does not mention `transliterate`
- **THEN** the system starts with case conversion on `Win+Alt+Pause`
- **AND** transliteration has no shortcut, and the system says so and names the combination it would have used

#### Scenario: The user writes the same binding twice

- **WHEN** the configuration sets both `change_case` and `transliterate` to `Win+Alt+Pause`
- **THEN** the configuration is rejected and the error names the line
