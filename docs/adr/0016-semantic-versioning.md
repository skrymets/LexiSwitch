# ADR-0016: Semantic versioning, declared once

## Status

Accepted. Supersedes nothing.

## Context

A running LexiSwitch could not say which version it was. The tray, the Diagnostics balloon
and the executable's File Properties were all silent, so a user reporting a problem could not
answer the first question asked of them, and a user who had just upgraded could not confirm
that the upgrade took.

The number existed. `[workspace.package] version` in the root `Cargo.toml` is inherited by all
three crates, and the release workflow already refuses a tag that disagrees with it. What did
not exist was anything downstream of that declaration — and a rule for what a change in it
means.

That rule needs stating because Semantic Versioning is written for libraries, where
"breaking change" means an API a caller compiled against stops working. LexiSwitch has no
callers. Its contract with the user is a configuration file they wrote by hand, a set of
shortcuts their hands have learned, and the behaviour behind those shortcuts.

## Decision

**The version follows Semantic Versioning 2.0.0 and is declared in exactly one place:
`[workspace.package] version` in the root `Cargo.toml`.** Everything else derives from it —
crate versions, embedded platform metadata, the version shown in the tray and in Diagnostics,
release titles and asset names. Nothing else holds a copy to keep in step.

What each component promises, for a program whose contract is a file and a set of keys:

- **MAJOR** — something the user relied on stops working as it did: a configuration file that
  was valid is now rejected or means something different, a default shortcut changes, or a
  feature is removed.
- **MINOR** — something is added and nothing that worked stops working: a new case style, a
  new option, a new tray command. A new configuration key with a default that preserves the old
  behaviour is MINOR.
- **PATCH** — a fix, with no change to anything the user configures or presses.

While the version is `0.x`, SemVer permits anything to change in any release. The same intent
applies shifted one place: MINOR for what would be MAJOR, PATCH for the rest.

A pre-release carries a hyphenated identifier (`0.2.0-rc.1`) and is published as a GitHub
pre-release. Build metadata (`+…`) is not used: the tag already names the commit, and two
builds of the same tag should not report different versions.

**Releases and artifacts are named from the version, not the tag.** The tag is `v<version>`,
because the workflow trigger needs the prefix. Files drop it:

- `lexiswitch-<version>-<target>.zip` — the target triple keeps a future second platform from
  colliding. The program inside keeps the stable name `lexiswitch.exe`, so unpacking a newer
  archive over an older one changes nothing that points at the file.
- `lexiswitch-<version>.exe` — the standalone program; the extension already names the platform.
- a `.sha256` beside each.

**Platform metadata carries the same version.** On Windows that is a version resource: numeric
`MAJOR.MINOR.PATCH.0`, the full version in the string fields, and the pre-release flag set when
there is an identifier the numeric field cannot hold. For ports that do not exist yet
([ADR-0001](0001-windows-first-with-os-free-core.md)), the mapping is recorded so that a port does
not have to rediscover it: macOS `CFBundleShortVersionString` = `MAJOR.MINOR.PATCH` and
`CFBundleVersion` = the full version; Linux package `Version:` = the full version, with the
pre-release separator the package format requires.

## Consequences

**Positive.** One edit changes every place the version appears, and the release gates check
that the binary agrees with the manifest, so a release cannot publish a program that misreports
itself. A user can find the version without the program running and while it runs. A version
number now says something about upgrading: a MINOR or PATCH release will accept the
configuration file the previous one accepted.

**Negative.** The rule puts a judgement on every release — is this change MAJOR? — that a date
or a counter would not. Release asset names change from the `v`-prefixed pattern of `0.1.0`, so
anything that guessed the old pattern must be updated; the old release keeps its names.

## Alternatives Considered

- **A separate `VERSION` file read by the build.** A second source that `Cargo.toml` would still
  have to agree with, since cargo requires a version of its own. Two places is the problem being
  solved.
- **Calendar versioning (`2026.10`).** Says when, not whether an upgrade is safe. For a program
  configured by hand, whether the file still works is the question users have.
- **Deriving the version from the latest git tag at build time.** Builds from an archive without
  history, or from a commit between tags, would report nothing or something misleading; and the
  release gate would be comparing the tag with itself.

## Related

- [ADR-0001](0001-windows-first-with-os-free-core.md) — why only Windows is built.
- [ADR-0008](0008-tray-and-toml-configuration.md) — the configuration file whose compatibility
  MAJOR protects.
- `DEV.md` — the release procedure, including the version bump.
