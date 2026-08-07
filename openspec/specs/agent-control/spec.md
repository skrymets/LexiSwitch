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

### Requirement: Only one instance runs at a time

At most one LexiSwitch SHALL run per user session. A second instance SHALL NOT start.

Detection SHALL be atomic, so that two instances started simultaneously cannot both conclude
they are the first.

A second instance SHALL NOT exit silently. It SHALL report that LexiSwitch is already running,
and SHALL ask the running instance to make itself visible to the user, so that a user who has
lost track of the existing one is shown where it is rather than left with a program that
appeared to do nothing.

Failing to start because an instance is already running SHALL NOT be reported as an error
condition: the user's intent was to have LexiSwitch running, and it is.

Where the running instance cannot be reached — because it is still starting, or is running with
privileges that prevent contacting it — the second instance SHALL still decline to start and
SHALL still say why.

#### Scenario: A second instance is started

- **WHEN** LexiSwitch is running and the user starts it again
- **THEN** the second instance does not start
- **AND** the user is told that LexiSwitch is already running
- **AND** the running instance is unaffected: its shortcut, its configuration and its tray icon are untouched

#### Scenario: The running instance is asked to show itself

- **WHEN** a second instance declines to start
- **THEN** the running instance makes itself visible to the user

#### Scenario: Two instances are started at the same moment

- **WHEN** two instances are started close enough together that both check before either has finished starting
- **THEN** exactly one of them runs

#### Scenario: The first instance has exited

- **WHEN** LexiSwitch is started after a previous instance has exited, including after a crash
- **THEN** it starts normally
- **AND** nothing left behind by the previous instance prevents it

### Requirement: A shortcut conflict is with another application

The shortcut-conflict state SHALL describe a combination held by a **different** application.

The system SHALL NOT present a conflict caused by another copy of itself, because that is not a
condition the user can usefully act on: choosing a different shortcut would leave two agents
running, which is the actual problem.

#### Scenario: The shortcut is held by another application

- **WHEN** the configured combination is registered by an application other than LexiSwitch
- **THEN** the conflict is reported and the combination named, as before

#### Scenario: The shortcut appears held by LexiSwitch itself

- **WHEN** a second instance would find the combination held by the running instance
- **THEN** the second instance declines to start rather than reporting a conflict

