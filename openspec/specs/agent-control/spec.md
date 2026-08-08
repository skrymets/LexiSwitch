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

The tray icon SHALL provide, at minimum: enable and disable, a command that invokes a
conversion, a command that invokes a case conversion, settings, reload configuration, open the
configuration file, diagnostics, and quit.

Disabling SHALL mean that the invocation hotkeys stop converting. It SHALL NOT be described as
pausing capture, because nothing is captured.

The tray SHALL distinguish at least these states, and SHALL show the active shortcuts where
they are registered: enabled, disabled, fewer than two layouts installed, shortcut conflict,
and runtime error.

Both invocation commands SHALL work whether or not the corresponding global shortcut is
registered, so that a shortcut conflict never leaves the product entirely unusable.

#### Scenario: User disables from the tray

- **WHEN** the user selects disable
- **THEN** both hotkeys stop converting
- **AND** the tray indicates the disabled state

#### Scenario: User invokes a conversion from the tray

- **WHEN** the user selects the conversion command with text selected in another application
- **THEN** a conversion transaction begins as though the shortcut had been pressed

#### Scenario: User invokes a case conversion from the tray

- **WHEN** the user selects the case-conversion command with text selected in another application
- **THEN** a case transaction begins as though the case shortcut had been pressed

#### Scenario: The shortcut is unavailable

- **WHEN** a configured shortcut cannot be registered
- **THEN** the tray shows the conflict state and names the combination
- **AND** both invocation commands remain usable

#### Scenario: User quits

- **WHEN** the user selects quit
- **THEN** hotkey registrations are released and the process exits
- **AND** no background process remains

### Requirement: Configuration is a readable file

Configuration SHALL be stored as a single human-readable text file that the user edits
directly. The file SHALL be the complete description of the system's **configurable
behavior**; the system SHALL NOT keep configurable state elsewhere.

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
- **THEN** every configurable behavior is visible in that one file
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

When a configured hotkey cannot be registered because another application holds it, the system
SHALL report the conflict and name the combination. The system SHALL NOT silently leave the
action unbound, and SHALL NOT substitute a different combination on the user's behalf.

Where more than one hotkey is configured, registration SHALL be independent per hotkey. A
hotkey that cannot be registered SHALL NOT prevent any other from being registered, and the
report SHALL name which combination is unavailable while making clear that the others are
working.

The system SHALL remain running. It SHALL NOT exit, and the conflict SHALL remain discoverable
until it is resolved.

The system SHALL provide a keyboard-operable way to record a replacement combination, validate
that it can be registered, and apply it without a restart. Where the replacement cannot be
registered, the system SHALL retain the previous working combination where one existed.

#### Scenario: Another application holds a hotkey

- **WHEN** the configured hotkey is already registered by another application
- **THEN** the system reports the conflict and names the combination
- **AND** the system keeps running with the conflict visible

#### Scenario: One of two hotkeys is unavailable

- **WHEN** one configured combination can be registered and the other cannot
- **THEN** the one that can is registered and works
- **AND** the report names the unavailable combination and states that the other is working

#### Scenario: User records a replacement shortcut

- **WHEN** the user records a new combination that can be registered
- **THEN** it becomes active without restarting the system
- **AND** the conflict state clears

#### Scenario: The replacement also conflicts

- **WHEN** the user records a combination that cannot be registered either
- **THEN** the system reports that it is unavailable and retains the previous working combination if there was one
- **AND** does not silently choose a third combination

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

### Requirement: State kept outside configuration records no content

Any state the system keeps outside the configuration file SHALL consist only of the system's
own identifiers and integers.

It SHALL NOT contain text the user typed or selected, any transformed result, any preview, any
application or window name, or any timestamp. The claim SHALL be checkable by opening the file
rather than by trusting a description of it.

#### Scenario: The state file is inspected

- **WHEN** the user opens a state file the system maintains
- **THEN** it contains only identifiers the system defines and integers
- **AND** nothing in it is derived from what the user typed

