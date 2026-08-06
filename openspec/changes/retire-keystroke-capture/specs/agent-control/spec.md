## MODIFIED Requirements

### Requirement: Tray controls

The tray icon SHALL provide, at minimum: enable and disable, reload configuration, open the
configuration file, and quit.

Disabling SHALL mean that the invocation hotkey stops converting. It SHALL NOT be described as
pausing capture, because nothing is captured.

#### Scenario: User disables from the tray

- **WHEN** the user selects disable
- **THEN** the hotkey stops converting
- **AND** the icon indicates the disabled state

#### Scenario: User quits

- **WHEN** the user selects quit
- **THEN** hotkey registrations are released and the process exits
- **AND** no background process remains

### Requirement: Configuration is a readable file

Configuration SHALL be stored as a single human-readable text file that the user edits
directly. The file SHALL be the complete description of the system's configurable behavior;
the system SHALL NOT keep configurable state elsewhere.

A configuration file written by an earlier version SHALL NOT prevent the system from starting.
Settings that no longer exist SHALL be ignored, and the system SHALL report which ones it
ignored, so that a removed setting is distinguishable from a misspelled one.

#### Scenario: User inspects configuration

- **WHEN** the user opens the configuration file
- **THEN** every configurable behavior is visible in that one file
- **AND** the file can be copied, diffed, and version-controlled

#### Scenario: Configuration file predates this version

- **WHEN** the system loads a configuration file containing settings that have been removed
- **THEN** the system starts, applying the settings it still recognises
- **AND** it reports each ignored setting by name

## REMOVED Requirements

### Requirement: Password-field protection cannot be disabled

**Reason**: This requirement exists so that the absence of a password-detection setting could
not be mistaken for an oversight — it made a safety property of keystroke capture
non-negotiable. With no capture there is no keystroke to withhold from a password field, so
the protection has nothing to protect and the setting has nothing to govern. Removing the
capture subsystem also resolves, rather than papers over, the measured unreliability of
password detection inside Chromium and Electron.

**Migration**: `respect_password_fields` is removed from configuration. A file still carrying
it starts normally under the migration behavior above, with the setting reported as ignored.

Note that this narrows what the program can protect the user from, and the narrowing is the
point: the program no longer sees typing at all, in a password field or anywhere else.
