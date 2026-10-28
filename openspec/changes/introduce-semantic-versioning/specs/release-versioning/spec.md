## Purpose

Where the program's version is declared, what it promises, and how every release, artifact name
and piece of platform metadata derives from that one declaration.

## ADDED Requirements

### Requirement: The version is declared once

The version SHALL be declared in exactly one place, the workspace manifest, and every crate SHALL
inherit it. Every artifact name, release title, piece of embedded platform metadata and in-program
display of the version SHALL derive from that declaration, so that changing it in one place
changes it everywhere.

The version SHALL be a Semantic Versioning 2.0.0 version, `MAJOR.MINOR.PATCH` optionally followed
by a hyphenated pre-release identifier.

#### Scenario: A developer bumps the version

- **WHEN** a developer changes the version in the workspace manifest and builds
- **THEN** the built program, its embedded metadata and any release made from it report the new version
- **AND** no other file had to be edited for that to be true

### Requirement: A release matches its version

A release SHALL be published only from a tag naming the version as `v<version>`, and only when
the workspace manifest at that tag declares exactly `<version>`. A mismatch SHALL stop the release
before anything is published.

A version with a pre-release identifier SHALL be published as a pre-release. The release SHALL be
titled `LexiSwitch <version>`.

#### Scenario: The tag disagrees with the manifest

- **WHEN** the tag `v0.2.0` is pushed while the manifest declares `0.1.0`
- **THEN** no release and no asset is published
- **AND** the failure names both versions

#### Scenario: A pre-release is tagged

- **WHEN** the tag `v0.2.0-rc.1` is pushed and the manifest declares `0.2.0-rc.1`
- **THEN** a release titled `LexiSwitch 0.2.0-rc.1` is published and marked as a pre-release

### Requirement: Artifact names carry the version

Each published file SHALL carry the version in its name, without the tag's leading `v`:

- an archive `lexiswitch-<version>-<target>.zip`, where `<target>` is the platform the program was
  built for;
- a standalone program `lexiswitch-<version>.exe`;
- beside each, a `.sha256` file naming it.

The program inside the archive SHALL keep the stable name `lexiswitch.exe`, so that upgrading by
unpacking a newer archive over an older one does not change the program's file name.

#### Scenario: Version 0.2.0 is released for Windows

- **WHEN** version `0.2.0` is released
- **THEN** the release holds `lexiswitch-0.2.0-x86_64-pc-windows-msvc.zip`, `lexiswitch-0.2.0.exe`, and a `.sha256` for each
- **AND** the archive contains `lexiswitch.exe`

### Requirement: The program carries platform version metadata

The built program SHALL carry the platform's own version metadata, so that the operating system's
file properties report the product name and version without the program running.

On Windows the program SHALL carry a version resource whose product name is `LexiSwitch`, whose
file and product version strings are the full version, and whose numeric file and product
version is `MAJOR.MINOR.PATCH.0`. When the version has a pre-release identifier, the resource
SHALL be flagged as a pre-release; the numeric version, having no room for the identifier, SHALL
omit it while the strings keep it.

A build whose embedded metadata does not match the declared version SHALL NOT be released.

#### Scenario: The user inspects the file

- **WHEN** the user opens the file properties of `lexiswitch.exe` built at version `0.2.0`
- **THEN** the product name is `LexiSwitch` and the product and file versions are `0.2.0`

#### Scenario: A pre-release is inspected

- **WHEN** the program is built at version `0.2.0-rc.1`
- **THEN** the product and file version strings are `0.2.0-rc.1`
- **AND** the numeric file version is `0.2.0.0`
- **AND** the file is flagged as a pre-release
