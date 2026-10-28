## Context

See `proposal.md` — Why. The policy itself (SemVer, what each component promises) is
[ADR-0016](../../../docs/adr/0016-semantic-versioning.md); this document is how it is carried
out.

Already in place and kept:

- `[workspace.package] version` in the root `Cargo.toml`, inherited by all three crates through
  `version.workspace = true`. This is where a Rust project's version lives; `cargo metadata`,
  `CARGO_PKG_VERSION` and every tool that reads a crate's version look here.
- The release workflow's **Tag matches Cargo.toml** gate, which compares the tag to
  `cargo metadata` and stops before anything is published.
- `lexiswitch/build.rs`, which writes a `.res` by hand rather than calling `rc.exe`, so that a
  build needs only cargo. Its `entry()` writer takes any resource type.

## Goals / Non-Goals

**Goals:**

- One edit — the manifest — changes every place the version appears.
- The version is discoverable without the program running (File Properties) and while it runs
  (tray menu, Diagnostics).
- A release whose binary misreports its version cannot be published.

**Non-Goals:**

- No `--version` flag. The release build uses the Windows subsystem and has no console to print
  to; a flag nobody can see is not a way to check the version.
- No build metadata (`+<commit>`) in the version. It would make two builds of the same tag
  differ, and the tag already names the commit.
- No macOS or Linux build. Only the mapping is recorded, below.
- No new dependency. The resource is written by the same few lines that write the icons.

## Decisions

**The version resource is written by `build.rs`, beside the icons.** Cargo gives build scripts
`CARGO_PKG_VERSION_MAJOR`, `_MINOR`, `_PATCH` and `_PRE`, so no parsing is needed. The resource
is one `RT_VERSION` entry: `VS_FIXEDFILEINFO` with the numeric version `M.m.p.0`, then a
`StringFileInfo` table (US English, Unicode code page) and a `VarFileInfo` translation naming the
same. Every block in that format is the same shape — length, value length, type, a UTF-16 key,
padding, a value, children — so one `node()` helper writes all of them and the layout rules live
in one place. A component above `u16::MAX` stops the build: the fixed field cannot hold it, and
truncating would publish a wrong version.

*Alternative considered:* the `winres`/`embed-resource` crates. Rejected for the reason
`build.rs` already gives — they need `rc.exe` or pull in a tree of build dependencies to avoid
it — and because the existing writer already does the hard part.

**`FileDescription` is `LexiSwitch`, not the crate description.** Task Manager lists a process
by its `FileDescription`, so the crate's one-line description there would name the running
process "Offline keyboard-layout correction for Windows 11." The description goes in `Comments`,
which Explorer shows under Details.

**Pre-release: numeric version without it, strings with it, and the flag set.** The fixed field
is four 16-bit numbers and has no room for `-rc.1`. Encoding the pre-release into the fourth
number would invent an ordering SemVer does not define. So `0.2.0-rc.1` is numerically `0.2.0.0`,
`VS_FF_PRERELEASE` is set, and `FileVersion`/`ProductVersion` read `0.2.0-rc.1`, which is what
Explorer displays.

**The tray menu gets a greyed first line, id 0.** `TrackPopupMenu` with `TPM_RETURNCMD` already
returns 0 for "nothing chosen", so an item with id 0 cannot be mistaken for a command even if it
were enabled; greying it says so visually as well. The text is built in the binary crate from
`env!("CARGO_PKG_VERSION")` and handed to `show_menu`, so `lexiswitch-win` does not decide what
the product calls itself.

**Asset names drop the `v`.** The tag keeps it (`on: push: tags: ['v*']` needs it); a file name is
the version, not the tag. The archive keeps the target triple so a future second platform does
not collide; the standalone `.exe` omits it because the extension already says Windows.

**The metadata gate reads what Windows reads.** After the release build, PowerShell's
`(Get-Item lexiswitch.exe).VersionInfo` is compared with `cargo metadata`: product version
string equal, numeric version equal to `M.m.p.0`, pre-release flag matching. This tests the
resource through the same API Explorer uses, rather than re-parsing bytes the build script wrote.
It runs on CI as well as on release, so a broken resource is found on the commit that broke it.

**Future platforms, recorded as rule.** If a port exists, the same manifest version maps to:

| Platform | Where                         | Value                                                            |
|----------|-------------------------------|------------------------------------------------------------------|
| macOS    | `Info.plist`                  | `CFBundleShortVersionString` = `M.m.p`; `CFBundleVersion` = full version |
| Linux    | package metadata (`.deb`/`.rpm`) | `Version:` = full version (pre-release separator per the format's rules) |
| any      | archive name                  | `lexiswitch-<version>-<target>.<ext>`                             |

## Risks / Trade-offs

- [Asset names change, so a script downloading `lexiswitch-v*-…zip` breaks] → only future
  releases change; the release notes and `DEV.md` show the new names. No automation in this repo
  depends on the old pattern.
- [A hand-written version resource has subtle alignment rules, and a wrong one is silently
  ignored by Explorer] → the metadata gate reads it back through the Windows API on every CI run;
  a resource Windows cannot read fails the build there, not on a user's machine.
- [The tray menu grows by two rows] → it is one line and a separator, and the version is the
  thing a user opening the menu to report a problem is looking for.
