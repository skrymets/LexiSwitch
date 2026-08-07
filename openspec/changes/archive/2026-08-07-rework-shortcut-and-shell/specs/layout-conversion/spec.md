## MODIFIED Requirements

### Requirement: Minimum of two layouts

Conversion requires at least two installed keyboard layouts, because a layout cannot be
converted to itself.

Where fewer than two are installed, the system SHALL start in a disabled state and SHALL
explain that at least two are required. It SHALL NOT exit, and SHALL NOT run in a state where
invoking it appears to do nothing.

Settings and diagnostics SHALL remain reachable in this state, so that a user who has just
been told what is required can act on it and does not have to reinstall or reconfigure blind.

The system SHALL become usable without a restart once a second layout is installed.

#### Scenario: Only one layout installed

- **WHEN** the system starts with fewer than two keyboard layouts installed
- **THEN** it starts in a disabled state that explains at least two layouts are required
- **AND** it does not exit
- **AND** settings and diagnostics remain reachable

#### Scenario: A second layout is installed while running

- **WHEN** the user installs a second layout while the system is running in the disabled state
- **THEN** the system becomes able to convert without being restarted

#### Scenario: Two or more layouts installed

- **WHEN** the system starts with two or more keyboard layouts installed
- **THEN** it builds conversion tables and enters the enabled state
