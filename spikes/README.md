# Spikes

Throwaway programs that answer a question. Nothing here ships.

**Nothing in this directory is a workspace member.** Each spike is a standalone cargo
project with its own `Cargo.toml` and its own lockfile, and the root workspace lists
`exclude = ["spikes"]`. That means `cargo build` and `cargo test --workspace` at the repo
root cannot compile or run anything here. The boundary is structural, not a convention —
see `openspec/changes/spike-windows-input-primitives/design.md` for why it is arranged that
way rather than as feature-gated modules inside `lexiswitch-win`.

**Quality rules do not apply.** `unwrap` is fine. Duplication between spikes is correct —
each one is meant to be readable alone, and factoring out a shared helper would create
exactly the coupling this directory exists to avoid. No error handling beyond what is
needed to see the result. No tests.

**The value of a spike is the answer, not the code.** Findings are recorded in
`docs/spikes/`, and that document is what later work cites. The only artifact that survives
from here is data — the frozen layout fixtures — never source.

To run one:

```powershell
cd spikes/spike3_layout_tables
cargo run
```

| Spike                  | Question                                                                                                |
|------------------------|---------------------------------------------------------------------------------------------------------|
| `spike3_layout_tables` | Can `GetKeyboardLayoutList` + `ToUnicodeEx` produce complete layout tables, including German dead keys? |
| `spike1_uipi_reach`    | Does `RIDEV_INPUTSINK` deliver keystrokes typed into elevated windows to an unelevated process?         |
| `spike2_hotkey_timing` | Do `WM_HOTKEY` timestamps resolve finely enough to distinguish tap from double-tap from triple-tap?     |

Spike 3 runs first and blocks the others on failure: it is the only one whose negative
result invalidates a decision outright ([ADR-0004](../docs/adr/0004-runtime-derived-layout-tables.md)).
