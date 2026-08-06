# agent-control Specification

## Purpose
How the user runs, configures, pauses, and troubleshoots a program that has no main window
and observes typing continuously — including the requirement that such a program be
visible while it runs.
## Requirements
### Requirement: The system is visibly present while running

The system SHALL show a tray icon for as long as it is running. The system SHALL NOT offer
an option to hide it. A program that observes keystrokes remains visible.

#### Scenario: System is running

- **WHEN** the system is running, whether active or paused
- **THEN** a tray icon is present
- **AND** its appearance distinguishes the active state from the paused state

### Requirement: Tray controls

The tray icon SHALL provide, at minimum: pause and resume, reload configuration, open the
configuration file, and quit.

#### Scenario: User pauses from the tray

- **WHEN** the user selects pause
- **THEN** keystroke recording stops, the buffer is wiped, and hotkeys stop converting
- **AND** the icon indicates the paused state

#### Scenario: User quits

- **WHEN** the user selects quit
- **THEN** hotkey registrations are released, the buffer is wiped, and the process exits
- **AND** no background process remains

### Requirement: Configuration is a readable file

Configuration SHALL be stored as a single human-readable text file that the user edits
directly. The file SHALL be the complete description of the system's configurable
behavior; the system SHALL NOT keep configurable state elsewhere.

#### Scenario: User inspects configuration

- **WHEN** the user opens the configuration file
- **THEN** every configurable behavior is visible in that one file
- **AND** the file can be copied, diffed, and version-controlled

### Requirement: Configuration reloads without restart

The system SHALL apply configuration changes without requiring a restart.

#### Scenario: User edits and saves configuration

- **WHEN** the user saves a valid change to the configuration file
- **THEN** the new configuration takes effect within a few seconds
- **AND** the system continues running throughout

### Requirement: Invalid configuration never leaves the system unconfigured

When configuration cannot be parsed or fails validation, the system SHALL continue running
under the last known good configuration and SHALL report the problem, identifying where in
the file it occurs.

#### Scenario: User saves a malformed configuration

- **WHEN** the configuration file is saved with a syntax error
- **THEN** the previously loaded configuration remains in effect
- **AND** the system reports the error and the location in the file

#### Scenario: Configuration is invalid at startup

- **WHEN** the configuration file is invalid and no previously loaded configuration exists
- **THEN** the system does not start
- **AND** it reports the error rather than starting with defaults silently substituted

### Requirement: Password-field protection cannot be disabled

The system SHALL reject any configuration that disables password-field detection.

#### Scenario: User attempts to disable the protection

- **WHEN** a configuration setting the password-field protection to disabled is loaded
- **THEN** validation fails and the configuration is rejected
- **AND** the system explains that the setting exists to be explicit, not to be turned off

### Requirement: Hotkey conflicts fail loudly

When a configured hotkey cannot be registered because another application holds it, the
system SHALL report the conflict and name the combination. The system SHALL NOT silently
leave the action unbound.

#### Scenario: Another application holds a hotkey

- **WHEN** a configured hotkey combination is already registered by another application
- **THEN** the system reports which combination is unavailable
- **AND** the user is directed to the configuration file to choose another

### Requirement: Failures reach the user

Every failure that prevents a requested action SHALL be reported through the tray. The
system SHALL NOT fail silently.

#### Scenario: An action cannot be completed

- **WHEN** a requested conversion fails for any reason
- **THEN** the system notifies the user
- **AND** the notification states what did not happen, not only that an error occurred

### Requirement: No network capability

The system SHALL NOT make network connections for any purpose, including update checking,
telemetry, analytics, and crash reporting. Absence of network capability SHALL be
verifiable mechanically rather than by inspection alone.

#### Scenario: System runs for an extended period

- **WHEN** the system runs while network activity is monitored
- **THEN** it makes no outbound connection

#### Scenario: Dependencies are checked

- **WHEN** the dependency check runs
- **THEN** it fails if any network-capable dependency is present

### Requirement: Logging is off by default and never records content

Logging SHALL be disabled unless explicitly enabled. When enabled, records SHALL describe
event kinds and sizes only.

#### Scenario: Logging is enabled and a correction occurs

- **WHEN** logging is enabled and the user corrects a word
- **THEN** the log records that a correction occurred and how many characters it affected
- **AND** it records neither the original nor the converted text

