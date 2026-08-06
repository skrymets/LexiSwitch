## MODIFIED Requirements

### Requirement: Tray controls

The tray icon SHALL provide, at minimum: enable and disable, a command that invokes a
conversion, settings, reload configuration, open the configuration file, diagnostics, and quit.

Disabling SHALL mean that the invocation hotkey stops converting. It SHALL NOT be described as
pausing capture, because nothing is captured.

The tray SHALL distinguish at least these states, and SHALL show the active shortcut where one
is registered: enabled, disabled, fewer than two layouts installed, shortcut conflict, and
runtime error.

The conversion command SHALL work whether or not the global shortcut is registered, so that a
shortcut conflict never leaves the product entirely unusable.

#### Scenario: User disables from the tray

- **WHEN** the user selects disable
- **THEN** the hotkey stops converting
- **AND** the tray indicates the disabled state

#### Scenario: User invokes a conversion from the tray

- **WHEN** the user selects the conversion command with text selected in another application
- **THEN** a conversion transaction begins as though the shortcut had been pressed

#### Scenario: The shortcut is unavailable

- **WHEN** the configured shortcut cannot be registered
- **THEN** the tray shows the conflict state and names the combination
- **AND** the conversion command remains usable

#### Scenario: User quits

- **WHEN** the user selects quit
- **THEN** hotkey registrations are released and the process exits
- **AND** no background process remains

### Requirement: Hotkey conflicts fail loudly

When a configured hotkey cannot be registered because another application holds it, the system
SHALL report the conflict and name the combination. The system SHALL NOT silently leave the
action unbound, and SHALL NOT substitute a different combination on the user's behalf.

The system SHALL remain running. It SHALL NOT exit, and the conflict SHALL remain discoverable
until it is resolved.

The system SHALL provide a keyboard-operable way to record a replacement combination, validate
that it can be registered, and apply it without a restart. Where the replacement cannot be
registered, the system SHALL retain the previous working combination where one existed.

#### Scenario: Another application holds a hotkey

- **WHEN** the configured hotkey is already registered by another application
- **THEN** the system reports the conflict and names the combination
- **AND** the system keeps running with the conflict visible

#### Scenario: User records a replacement shortcut

- **WHEN** the user records a new combination that can be registered
- **THEN** it becomes active without restarting the system
- **AND** the conflict state clears

#### Scenario: The replacement also conflicts

- **WHEN** the user records a combination that cannot be registered either
- **THEN** the system reports that it is unavailable and retains the previous working combination if there was one
- **AND** does not silently choose a third combination

### Requirement: Configuration is a readable file

Configuration SHALL be stored as a single human-readable text file that the user edits
directly. The file SHALL be the complete description of the system's configurable behavior;
the system SHALL NOT keep configurable state elsewhere.

A configuration file written by an earlier version SHALL NOT prevent the system from starting.
Settings that no longer exist SHALL be ignored, and the system SHALL report which ones it
ignored, so that a removed setting is distinguishable from a misspelled one.

Settings the user is expected to change during normal use — the invocation shortcut and
whether the system is enabled — SHALL also be reachable from the interface, without requiring
the user to find and edit the file.

#### Scenario: User inspects configuration

- **WHEN** the user opens the configuration file
- **THEN** every configurable behavior is visible in that one file
- **AND** the file can be copied, diffed, and version-controlled

#### Scenario: Configuration file predates this version

- **WHEN** the system loads a configuration file containing settings that have been removed
- **THEN** the system starts, applying the settings it still recognises
- **AND** it reports each ignored setting by name

#### Scenario: User changes the shortcut without editing the file

- **WHEN** the user records a new shortcut through the interface
- **THEN** the change takes effect and is persisted to the configuration file
