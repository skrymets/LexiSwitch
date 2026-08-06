## Purpose

Retains just enough recent typing to reconstruct the last word for correction, under
containment rules strict enough that the retention is defensible — this is the only
component that holds data passively, so its rules are the runtime privacy model.

## ADDED Requirements

### Requirement: Observation cannot block or alter input

The system SHALL observe keystrokes only through a mechanism that has no capability to
suppress, delay, or modify them before the focused application receives them. The system
SHALL NOT install a keyboard hook, and SHALL NOT provide an abstraction that would make
installing one a configuration change.

#### Scenario: Auditor inspects the binary

- **WHEN** a reader examines which operating system input APIs the program imports
- **THEN** no keystroke-interception API is present
- **AND** the absence of interception is evident from the imports, without reading program
  logic

#### Scenario: System is busy processing a correction

- **WHEN** a conversion is in progress and takes several seconds
- **THEN** typing in every application continues at normal speed
- **AND** no keystroke is delayed, dropped, or reordered by the system's presence

### Requirement: The buffer is bounded and memory-resident

The system SHALL retain at most 64 recent keystrokes. The buffer SHALL exist only in
memory, SHALL be excluded from paging to disk, and SHALL be overwritten when discarded.

#### Scenario: Typing exceeds the buffer size

- **WHEN** more than 64 keystrokes are typed without an intervening wipe
- **THEN** the oldest entries are discarded
- **AND** at most 64 entries are retained at any moment

#### Scenario: Buffer is discarded

- **WHEN** the buffer is wiped for any reason
- **THEN** its contents are overwritten rather than merely marked unused

### Requirement: Nothing derived from keystrokes reaches disk

The system SHALL NOT write keystrokes, or any text derived from them, to any file. This
applies to configuration, logs, crash dumps, caches, and temporary files without
exception.

#### Scenario: Logging is enabled

- **WHEN** the user enables logging and performs a correction
- **THEN** the log records the kind of event and the length of the text
- **AND** the log contains no characters from the converted text

#### Scenario: System is inspected after a session of use

- **WHEN** every file the system has written is examined after extended use
- **THEN** none contains typed text or anything from which it could be reconstructed

### Requirement: The buffer does not fill in unsafe contexts

The system SHALL NOT record keystrokes while the focused control is a password field,
while the foreground application matches the configured exclusion list, or while the
system is paused.

#### Scenario: Password field is focused

- **WHEN** the focused control reports itself as a password field and the user types
- **THEN** no keystroke is recorded
- **AND** a subsequent correction request finds nothing to correct

#### Scenario: Excluded application is in the foreground

- **WHEN** the foreground application appears in the configured exclusion list and the
  user types
- **THEN** no keystroke is recorded
- **AND** the correction hotkeys remain registered and continue to function elsewhere

#### Scenario: System is paused

- **WHEN** the user pauses the system and types
- **THEN** no keystroke is recorded
- **AND** the buffer is wiped at the moment of pausing rather than retained for resumption

### Requirement: The buffer is wiped at word and context boundaries

The system SHALL discard the buffer when the foreground window changes, when `Enter`,
`Tab`, or `Escape` is pressed, when the configured idle timeout elapses with no typing,
and after any successful conversion.

#### Scenario: User switches applications

- **WHEN** the foreground window changes
- **THEN** the buffer is wiped before any keystroke in the new window is recorded

#### Scenario: User stops typing

- **WHEN** the configured idle timeout elapses with no keystroke
- **THEN** the buffer is wiped

#### Scenario: Correction completes

- **WHEN** a correction is successfully applied
- **THEN** the buffer is wiped, so a later hotkey press does not re-correct stale content

### Requirement: The buffer records key positions, not characters

The system SHALL retain physical key identity and shift state rather than resolved
characters. Resolution to text SHALL occur only when a conversion is performed.

#### Scenario: Buffer contents are examined in memory

- **WHEN** the buffer's contents are inspected directly
- **THEN** they consist of key positions and shift states
- **AND** interpreting them as text requires a layout table
