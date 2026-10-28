## Why

A running LexiSwitch has no way to say which version it is. The tray menu, the Diagnostics
balloon and the executable's File Properties are all silent, so a user reporting a problem — or
checking whether an upgrade took — cannot answer the first question anyone will ask. Half the
plumbing already exists: the version is declared once in the workspace manifest and the release
workflow refuses a tag that disagrees with it. What is missing is everything downstream of that
declaration, and a written rule for what a version number promises.

## What Changes

- Adopt Semantic Versioning 2.0.0 as policy, recorded in ADR-0016, including what MAJOR, MINOR
  and PATCH mean for a program whose contract is a configuration file and a set of shortcuts
  rather than an API.
- Keep `[workspace.package] version` in the root `Cargo.toml` as the single place a developer
  sets the version. Every crate, artifact name, release title and embedded metadata derives from
  it.
- Embed Windows version metadata in `lexiswitch.exe`, so Explorer → Properties → Details shows
  the product name and version. A pre-release is flagged as one and carries its full version
  string.
- Show the version in the running program: a non-actionable `LexiSwitch <version>` line at the
  top of the tray menu, and the version in the Diagnostics balloon.
- Name release assets by version: `lexiswitch-<version>-x86_64-pc-windows-msvc.zip` (the
  program inside keeps the stable name `lexiswitch.exe`) and a standalone
  `lexiswitch-<version>.exe`, each with a `.sha256`. The leading `v` leaves asset names; tags
  keep it. Releases are titled `LexiSwitch <version>`.
- Gate releases and CI on the embedded metadata matching the manifest.
- Document how the same version maps onto macOS and Linux metadata, as the rule for whenever
  those ports exist. Nothing is built for them (ADR-0001).

## Capabilities

### New Capabilities

- `release-versioning`: where the version is declared, how releases, artifact names and
  platform metadata derive from it, and what the version number promises.

### Modified Capabilities

- `agent-control`: the running program shows its version in the tray menu and in Diagnostics.

## Impact

- `lexiswitch/build.rs`: a version resource alongside the icons, in the same hand-written `.res`.
- `lexiswitch/src/main.rs`, `lexiswitch-win/src/tray.rs`: the version line and the Diagnostics
  text. `show_menu` gains a parameter.
- `.github/workflows/release.yml`, `.github/workflows/ci.yml`: metadata gate, asset names,
  release title.
- `DEV.md`, `packaging/INSTALL.txt`, `README.md`, `docs/testing/manual-checklist.md`.
- New `docs/adr/0016-semantic-versioning.md`.
- No new dependency. Release asset names change, which breaks any link to a previous asset
  *pattern* — existing releases keep their names.
