# Development Environment

What must be present to build, test, and verify LexiSwitch. Surveyed on the development
machine on 2026-08-05.

## Status at a glance

| Requirement                                 | State          | Action                                   |
|---------------------------------------------|----------------|------------------------------------------|
| Windows 11 (10.0.26200)                     | ✅ present     | —                                        |
| Rust 1.97.1 `stable-x86_64-pc-windows-msvc` | ✅ present     | —                                        |
| `clippy`, `rustfmt`, `rust-src`             | ✅ present     | —                                        |
| MSVC linker — VS Build Tools 2022           | ✅ present     | —                                        |
| Windows SDK 10.0.22621.0                    | ✅ present     | —                                        |
| `inspect.exe` (UI Automation explorer)      | ✅ present     | —                                        |
| Git 2.55.0, Node v25.6.1, OpenSpec 1.7.0    | ✅ present     | —                                        |
| `%USERPROFILE%\.cargo\bin` on `PATH`        | ✅ present     | —                                        |
| German keyboard layout (`0407`)             | ✅ present     | —                                        |
| `cargo-deny` 0.20.2, `cargo-audit` 0.22.2   | ✅ present     | —                                        |
| RustSec advisory database                   | ⚠️ local copy  | run offline; refreshing needs §3         |
| Manual test matrix applications             | ⚠️ unverified  | confirm before task 7.4                  |

Installed keyboard layouts are `en-US` (`0409:00000409`), `uk` (`0422:00020422`), and
`de-DE` (`0407:00000407`) — the full target set.

---

## 1. Build toolchain

**Rust `stable-x86_64-pc-windows-msvc`.** The MSVC target, not GNU — the `windows` crate
and the Win32 linking model assume it, and mixing toolchains produces link errors that
look like missing symbols.

Components needed beyond the default: `clippy` and `rustfmt` (both present). `rust-src`
is present and is what gives RustRover standard-library navigation.

**MSVC linker and Windows SDK.** Rust does not ship a linker on Windows; it uses
`link.exe` from Visual Studio Build Tools, and the SDK supplies the import libraries for
every Win32 call in `lexiswitch-win`. Build Tools 2022 with the "Desktop development with
C++" workload covers both.

**`PATH`.** `%USERPROFILE%\.cargo\bin` is on the user `PATH`. It appears twice — the
second occurrence, at the end, carries a stray leading space (`; C:\Users\serga\.cargo\bin`)
from an earlier append. Harmless, but do not "fix" this by appending again; check first:

```powershell
[Environment]::GetEnvironmentVariable('PATH', 'User') -like '*\.cargo\bin*'
```

A shell opened before the entry was added will not have it. That looks identical to the
entry being absent, and the fix is a new terminal, not another append. Confirm with
`cargo --version`.

---

## 2. Keyboard layouts — a hard test dependency

This is not optional tooling. Layout tables are derived at runtime from installed layouts
([ADR-0004](adr/0004-runtime-derived-layout-tables.md)), so the layouts installed on this
machine determine what can be built and tested at all:

- **Ukrainian** — installed. The primary source layout.
- **English (US)** — installed. The primary target layout.
- **German (Germany)** — installed, `0407:00000407`. This is the standard QWERTZ layout,
  which is the one that matters: spike 3 must extract a `de` table, task 2.10's regression
  corpus covers German umlauts and QWERTZ `y`/`z` transposition, and German is the only
  layout in the set with dead keys (`^`, `´`) — the specific hazard the spike exists to
  expose. Had it stayed uninstalled, the riskiest part of layout extraction would have gone
  untested.

To add a layout: Settings → Time & language → Language & region → German → Keyboard.

The program also refuses to start with fewer than two layouts installed, so the minimum
for running anything at all is two.

---

## 3. Verification tooling

**`cargo-deny` 0.20.2 and `cargo-audit` 0.22.2** — the no-network property is enforced
mechanically rather than by convention, and these are what enforce it. Both installed with:

```powershell
cargo install cargo-deny cargo-audit --locked
```

`cargo deny check bans licenses sources` passes against the workspace. The deny list in
`deny.toml` has been exercised against a real `reqwest` dependency and rejects it four ways
— by name, transitively via `hyper`, at the `socket2`/`mio` chokepoint, and through the
banned `tokio` `net` feature. Task 1.4 asks for that proof in the repo; it was done in a
throwaway project outside it, so redo it here if the task's wording is meant literally.

**The advisory database works, but only offline, and this needs understanding rather than
copying.** Both tools hold a local copy, populated once on 2026-08-05. Run them like this:

```powershell
cargo deny --offline check advisories   # reads %USERPROFILE%\.cargo\advisory-dbs\
cargo audit -n                          # reads %USERPROFILE%\.cargo\advisory-db\
```

Both flags mean the same thing — *do not fetch* — and both are load-bearing. Drop them and
the command tries to reach github and fails. The two tools keep **separate** copies in
separate directories, so populating one does nothing for the other.

Why fetching fails: **Avast is intercepting HTTPS.** It presents a certificate for
`github.com` issued by `CN=Avast Web/Mail Shield Root`, a self-signed root it has installed
into both Windows certificate stores. Git is configured with `http.sslBackend = openssl`,
whose bundle contains real public CAs and not Avast's, so verification fails. Everything
using the Windows trust store — Chrome, .NET, and cargo itself — accepts it silently, which
is why `cargo install` worked and only git complains.

Note what that does and does not mean: OpenSSL is *detecting* the interception, not
preventing it. The traffic is decrypted by Avast either way.

**The caveat that matters more than the workaround.** A frozen advisory database degrades
into theater: `advisories ok` against a copy from months ago means "ok as of the day it was
cloned", and it will keep saying `ok` indefinitely while saying nothing about anything
published since. Refresh before trusting a clean result, and record the date when a result
is used as evidence — task 10.2 in particular. Refreshing needs the interception accepted
for the duration of one command:

```powershell
# cargo-audit's copy.
git -c http.sslBackend=schannel -C "$env:USERPROFILE\.cargo\advisory-db" pull

# cargo-deny's copy. It shells out to git, so per-shell config reaches it; this
# writes nothing persistent and evaporates with the shell.
$env:GIT_CONFIG_COUNT=1
$env:GIT_CONFIG_KEY_0='http.sslBackend'
$env:GIT_CONFIG_VALUE_0='schannel'
cargo deny check advisories             # note: no --offline, so it fetches
```

The permanent alternatives are `git config --global http.sslBackend schannel` (accepts the
interception everywhere), or turning off Avast's HTTPS scanning under Menu → Settings →
Protection → Core Shields → Web Shield (removes the interception, machine-wide). Neither is
a LexiSwitch decision — the product makes no network connections and its threat model is
untouched. This only ever affected two build-time tools.

**`inspect.exe`** — ships with the Windows SDK, present at
`C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\inspect.exe`. Used to explore
what UI Automation actually exposes in a given application: whether a control reports
`IsPassword`, and whether a text pattern is available. This is the tool that answers task
7.4's coverage question and informs the selection-conversion work in task 8.1. Run it
before writing the code, not after it misbehaves.

**Network monitoring** for task 10.5 — confirming no outbound connection during an
extended session. Windows Resource Monitor (`resmon`) is built in and sufficient.
Sysinternals TCPView is more convenient if you already use it; neither is required beyond
one of them existing.

**Memory measurement** for task 10.4 — `Get-Process lexiswitch` reports the working set.
No extra tooling needed.

---

## 4. Manual test matrix

Tasks 7.4, 8.6, and 10.7 test behavior that cannot be covered by automated tests: password
field detection, selection conversion across accessibility implementations, and injection
into real applications. Each application in this list exists to exercise a distinct code
path, not for redundancy:

| Application                               | What it exercises                                                          |
|-------------------------------------------|----------------------------------------------------------------------------|
| Notepad                                   | Native Win32 edit control — the baseline where everything should work      |
| Windows Terminal                          | No usable text pattern; forces the clipboard fallback                      |
| VS Code                                   | Electron with a good accessibility implementation                          |
| Chrome or Firefox                         | Browser password fields — the weakest point in detection                   |
| A password manager (KeePassXC, 1Password) | Native password field, and the case where a detection failure matters most |
| Telegram or similar                       | Electron chat client, the common real-world typing target                  |

Confirm what is installed before task 7.4. A gap in the matrix means a gap in the
password-detection coverage measurement, which is the one measurement the privacy model
depends on.

---

## 5. Administrative access

Spike 1 requires launching an elevated Notepad or Windows Terminal and typing into it, to
determine whether Raw Input observes keystrokes in elevated windows from an unelevated
process. Local administrator rights are needed for that one test.

LexiSwitch itself runs unelevated.

---

## 6. IDE

RustRover, serving two MCP endpoints. Both are registered for this project at local scope,
so the global `intellij-index` / `intellij-debugger` entries pointing at IntelliJ IDEA
(29170 and 29190) are untouched:

| Server               | Endpoint                                              | Use                               |
|----------------------|-------------------------------------------------------|-----------------------------------|
| `rustrover-index`    | `http://127.0.0.1:29178/index-mcp/streamable-http`    | Code navigation and refactoring   |
| `rustrover-debugger` | `http://127.0.0.1:29198/debugger-mcp/streamable-http` | Interacting with the IDE debugger |

Both ports are served by the same RustRover process. The IDEA entries refuse connections
whenever IDEA is not running, which looks identical to a broken RustRover setup — check
the port before concluding anything is wrong. Registering a server does not make its tools
available to an already-running Claude Code session; that needs a restart or `/mcp`.

Not a build requirement — everything builds from the command line.

---

## 7. Deliberately not required

- **Code signing certificate.** Not needed for a locally built binary. Reconsider only if
  LexiSwitch is ever distributed.
- **Installer tooling.** v1 is a single executable plus a config file.
- **Cross-compilation targets.** Windows only
  ([ADR-0001](adr/0001-windows-first-with-os-free-core.md)).
- **A network connection at runtime.** Required to fetch crates at build time; the built
  program never makes one.
- **Dictionaries or word lists.** Deferred to Phase 3
  ([ADR-0007](adr/0007-manual-only-v1.md)).

---

## 8. Defender and EDR

**Avast is installed on this machine** (established while diagnosing the advisory-database
failure in §3 — its Web/Mail Shield root certificate is in both Windows certificate
stores). That makes the paragraph below concrete rather than hypothetical: third-party AV
is present, and it is the kind of product that reacts to this class of program.

Raw Input registration draws far less heuristic attention than a low-level keyboard hook
([ADR-0003](adr/0003-raw-input-over-low-level-hook.md)), but a program that registers
global hotkeys and calls `SendInput` may still be flagged by aggressive endpoint
protection — particularly an unsigned debug binary run from a build directory.

If that happens, add an exclusion for the build output directory — in Avast as well as
Defender — rather than disabling protection. Note it in `docs/manual-tests.md` if it affects a test result, since
"the correction did not apply" and "the binary was quarantined mid-test" look identical
from the outside.
