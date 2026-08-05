# ADR-0002: Rust as the implementation language

## Status

Accepted

## Context

LexiSwitch must run continuously as a background agent, hold a buffer of recent
keystrokes in memory, and call directly into Win32 input, automation, and clipboard APIs.

Two hard constraints: binary under 5 MB and resident memory under 20 MB. Managed runtimes
are excluded by arithmetic — Electron ships a browser, and .NET, Java, and Python each
carry a runtime that exceeds the entire budget before any project code exists.

That leaves systems languages. Two properties matter beyond size:

- The process holds keystroke data. A memory-safety defect here is not an ordinary crash;
  it is a disclosure of exactly the material this project exists to protect.
- The code is auditable by design. Its trustworthiness rests on someone being able to
  read it and conclude it does nothing else.

## Decision

**Rust**, 2021 edition, with the `windows` crate for Win32 bindings.

`#![forbid(unsafe_code)]` on `lexiswitch-core`. All `unsafe` confined to `lexiswitch-win`,
each block carrying a comment stating the invariant it upholds.

## Consequences

**Positive.** Buffer handling, the one place a memory defect would be a security incident,
is checked by the compiler rather than by review. `Send`/`Sync` make the two-thread design
(message loop plus worker) safe to express rather than merely conventional. Release
binaries with LTO and `panic = "abort"` land in the 1.5–2.5 MB range, well inside budget.
`cargo-deny` and `cargo-audit` give a mechanical way to enforce the no-network property
(ADR-0003 and the spec) instead of relying on discipline.

**Negative.** The `windows` crate's generated bindings are verbose, and its feature flags
must be pared deliberately or compile times and binary size both suffer. Win32 patterns —
message loops, `WNDPROC` callbacks, COM lifetime for UI Automation — are documented in
C++ and require translation. Rust binaries also carry more baseline size than equivalent
C++, though the margin here is irrelevant against a 5 MB budget.

## Alternatives Considered

**C++20.** Smaller binaries (~500 KB–2 MB), lower baseline RAM, and Win32 documentation
applies without translation. Rejected on the security argument: manual lifetime management
around a keystroke buffer and COM interfaces is precisely where use-after-free and leak
defects appear, and for this program those defects have a disclosure consequence rather
than a crash consequence. The size advantage buys nothing against a budget already met.

**C# / .NET AOT.** Native AOT does produce standalone binaries, but they start around
10–15 MB for a tray application, and P/Invoke marshalling around Raw Input and UI
Automation adds a layer to audit rather than removing one. Over budget.

**Go.** Good concurrency story, but ~8–10 MB binaries with a runtime and GC, and cgo-free
Win32 access is awkward for callback-driven APIs like `WNDPROC`. Rejected.

## Related

- [ADR-0001](0001-windows-first-with-os-free-core.md) — crate structure
- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — what memory safety is protecting
- [Change: add-manual-layout-conversion](../../openspec/changes/add-manual-layout-conversion/design.md)
