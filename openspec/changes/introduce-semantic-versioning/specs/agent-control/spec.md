## ADDED Requirements

### Requirement: The running version is visible

The tray menu SHALL show `LexiSwitch <version>` as its first line, in every state. That line SHALL
NOT be a command: choosing it SHALL do nothing.

Diagnostics SHALL include the version.

The version shown SHALL be the same string as the program's release and its embedded platform
metadata.

#### Scenario: User opens the tray menu

- **WHEN** the user opens the tray menu of version `0.2.0`
- **THEN** the first line reads `LexiSwitch 0.2.0`
- **AND** choosing it closes the menu and does nothing else

#### Scenario: User asks for diagnostics

- **WHEN** the user selects Diagnostics
- **THEN** the information shown includes the version
