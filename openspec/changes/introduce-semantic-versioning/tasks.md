## 1. Policy

- [x] 1.1 Write `docs/adr/0016-semantic-versioning.md`: SemVer 2.0.0, the workspace manifest as the single source, what MAJOR/MINOR/PATCH mean for this program, how `0.x` is treated, the asset naming rule and the per-platform metadata mapping.

## 2. Version metadata in the binary

- [x] 2.1 `lexiswitch/build.rs`: write an `RT_VERSION` resource from `CARGO_PKG_VERSION_*` — fixed info `M.m.p.0`, `VS_FF_PRERELEASE` when there is a pre-release, string table (`FileDescription` `LexiSwitch` so Task Manager names the process, the crate description in `Comments`) and translation — through one `node()` helper and the existing `entry()`. Refuse a component above `u16::MAX`. Update the module doc.
- [x] 2.2 Release build, then `(Get-Item target\release\lexiswitch.exe).VersionInfo` shows ProductName `LexiSwitch`, ProductVersion and FileVersion `0.1.0`, `IsPreRelease` false.
- [x] 2.3 Temporarily set the version to `0.2.0-rc.1`, rebuild, confirm the strings read `0.2.0-rc.1`, the numeric version is `0.2.0.0` and `IsPreRelease` is true; restore the version and `Cargo.lock`.

## 3. Version in the running program

- [x] 3.1 `lexiswitch-win/src/tray.rs`: `show_menu` takes the title and draws it greyed, id 0, as the first item followed by a separator. SAFETY comment kept accurate.
- [x] 3.2 `lexiswitch/src/main.rs`: build the title from `CARGO_PKG_VERSION`, pass it to `show_menu`, and add the version to the Diagnostics balloon. Update any example that calls `show_menu`.

## 4. Release and CI

- [x] 4.1 `release.yml`: a **Version metadata** gate after the build, comparing `VersionInfo` with the tag.
- [x] 4.2 `release.yml`: asset names `lexiswitch-<version>-x86_64-pc-windows-msvc.zip` and `lexiswitch-<version>.exe`, a `.sha256` for each, title `LexiSwitch <version>`, preamble naming both downloads.
- [x] 4.3 `ci.yml`: the same metadata gate, against `cargo metadata`.

## 5. Documentation

- [x] 5.1 `DEV.md`: new asset names in the procedure, the SemVer rule pointing at ADR-0016, and the local `VersionInfo` check.
- [x] 5.2 `packaging/INSTALL.txt`: how to tell which version is running.
- [x] 5.3 `README.md`: the version line in the tray menu.
- [x] 5.4 `docs/testing/manual-checklist.md`: menu header shows the version and does nothing when chosen; Diagnostics shows it; File Properties → Details shows it.

## 6. Verification

- [x] 6.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo deny check bans licenses sources`, `cargo deny --offline check advisories`, `cargo audit -n`, `cargo build --release` under 5 MB with no warnings.
- [x] 6.2 Run the package step's naming logic locally with `TAG=v0.1.0` and confirm the four asset names.
- [ ] 6.3 Manual: right-click the tray icon of a release build — first line `LexiSwitch 0.1.0`, greyed, choosing it does nothing; Diagnostics shows the version; Explorer Properties → Details shows it. Record in `docs/manual-tests.md`.
