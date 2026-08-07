## ADDED Requirements

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
