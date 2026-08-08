# Development and release

Everything a maintainer does that is not writing code. For how the program is put together
see `CLAUDE.md` and `docs/adr/`; for how the machine is set up see `docs/environment.md`.

## The two workflows

| File                            | Runs on                                   | Does                                                                                        |
|---------------------------------|-------------------------------------------|---------------------------------------------------------------------------------------------|
| `.github/workflows/ci.yml`      | every push to `develop`, every PR into it | clippy, tests, `cargo deny`, `cargo audit`, a release build, and reports the binary size    |
| `.github/workflows/release.yml` | a pushed tag matching `v*`                | the same gates, plus the tag/version check and the 5 MB budget, then packages and publishes |

Both run on `windows-latest`. A Linux runner cannot compile this workspace — `lexiswitch-win`
is Win32 FFI top to bottom and `cargo test --workspace` builds it.

The gates are duplicated on purpose. CI catches a regression on the branch, where fixing it
is a commit; the release re-runs them on the exact tagged tree, so nothing that was merged
after the last green CI run can slip into a published binary.

## Releasing, step by step

The tag is the release. Pushing `v0.2.0` is the only action that publishes anything, and
there is no manual button — so every version that shipped exists in git history under exactly
that name.

### 1. Start from a green branch

```powershell
git switch develop
git pull
```

Check the [Actions tab](https://github.com/skrymets/LexiSwitch/actions) — the newest run on
`develop` should be green. Tagging a red branch just moves the failure later.

### 2. Bump the version

One place only: `version` under `[workspace.package]` in the root `Cargo.toml`. All three
crates inherit it with `version.workspace = true`.

```toml
[workspace.package]
version = "0.2.0"
```

Then refresh the lockfile, which carries the same three version numbers:

```powershell
cargo check --workspace
```

### 3. Run the gates locally

Optional — CI runs all of these — but a five-minute local run beats a failed release.

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check bans licenses sources
cargo deny --offline check advisories
cargo audit -n
cargo build --release
```

`--offline` and `-n` are required **here and not on CI**. Avast intercepts HTTPS on this
machine, so fetching the RustSec database fails TLS verification and both tools fall back to
their local copy (`docs/environment.md` §3). Runners have no such interception, so CI fetches
the database fresh — meaning CI's advisory check is the current one and yours is only as good
as the last refresh. Do not "fix" a local TLS error by weakening verification.

### 4. Commit and push

```powershell
git add Cargo.toml Cargo.lock
git commit -m "Release 0.2.0"
git push
```

Wait for CI to go green before tagging. Everything up to here is undoable with another
commit; the tag is the point where that stops being true.

### 5. Tag and push the tag

```powershell
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

The leading `v` matters — `on: push: tags: ['v*']` is what starts the workflow, and a tag
named `0.2.0` does nothing at all.

### 6. Watch it

The run appears under Actions within a few seconds and takes roughly five minutes. In order,
it will:

1. check the tag against `Cargo.toml` — `v0.2.0` against `0.2.0`
2. run clippy, the tests, `cargo deny` and `cargo audit`
3. build release and fail if `lexiswitch.exe` exceeds 5 MB
4. stage `lexiswitch.exe`, `packaging/INSTALL.txt` and `config.example.toml` into
   `lexiswitch-v0.2.0-x86_64-pc-windows-msvc.zip`, and write its `.sha256` beside it
5. create the release with an install preamble followed by GitHub's generated list of
   commits since the previous tag

Steps 1–3 are all before anything is published, so a failure leaves no release object and no
assets — only a tag to delete.

### 7. Check the result

Download the zip from the release page and confirm the hash matches the `.sha256` asset:

```powershell
(Get-FileHash lexiswitch-v0.2.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash.ToLower()
```

Then unzip it somewhere clean and run it once. The workflow proves it builds; only you can
prove it starts.

## When a release fails

Nothing was published, so the fix is to remove the tag, correct the problem, and tag again.

```powershell
git push origin :refs/tags/v0.2.0   # delete it on the remote first
git tag -d v0.2.0                   # then locally
```

Reusing the same version number is fine as long as nobody could have downloaded it — and
nobody could, because the gates run before the release is created. If a release object *did*
get created (you got as far as step 5 and something later went wrong), delete it from the
release page before re-tagging; `gh release create` will not overwrite one.

### The failure that actually happens

> Tag v0.2.0 wants version 0.2.0, but Cargo.toml has 0.1.0.

Step 2 was skipped or its commit was not pushed. Delete the tag, do step 2, commit, push,
tag again.

## Pre-releases

A tag with a hyphen in it is marked as a prerelease automatically, so it does not become the
"Latest release" on the repository front page:

```powershell
git tag -a v0.2.0-rc.1 -m "v0.2.0-rc.1"
git push origin v0.2.0-rc.1
```

The version check compares against the whole string after the `v`, so `Cargo.toml` has to say
`version = "0.2.0-rc.1"` for that tag to pass.

## Known gap: pinning the third-party actions

Three actions are referenced by major-version tag rather than by commit SHA:

- `actions/checkout@v4` — GitHub's own
- `Swatinem/rust-cache@v2` — third party
- `taiki-e/install-action@v2` — third party

A tag is mutable. `deny.toml` refuses wildcard crate versions on the grounds that a crate can
be taken over by whoever inherits it tomorrow, and the same argument applies to an action
whose tag its author can move at will — so the two third-party ones should be pinned to full
commit SHAs. They are not, because the SHAs could not be resolved from this machine (`git
ls-remote` hits the same Avast TLS interception as everything else) and a guessed SHA is
worse than an honest tag.

To close it, from any machine with working TLS:

```bash
git ls-remote --tags https://github.com/Swatinem/rust-cache | grep '\^{}$'
git ls-remote --tags https://github.com/taiki-e/install-action | grep '\^{}$'
```

Take the commit SHA for the newest version tag (the `^{}` line dereferences the annotated tag
to its commit) and write it into both workflow files with the version beside it:

```yaml
- uses: Swatinem/rust-cache@<40-hex-sha> # v2.8.0
```

## What is not automated

- **Code signing.** The binary is unsigned, which is why the release ships a SHA-256 and
  `packaging/INSTALL.txt` tells the user to check it. A certificate is the only real fix and
  costs money; SmartScreen reputation accrues on its own with downloads.
- **A changelog.** Release notes are GitHub's generated commit list under a fixed install
  preamble. Commits are feature-sized and describe the change, which is what makes that list
  readable — keep writing them that way and there is nothing else to maintain.
- **`winget` / Chocolatey.** No package manifest anywhere. "No installer" is a stated property
  of the program, and a manifest is the first step towards arguing about it.
