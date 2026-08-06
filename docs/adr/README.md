# Architecture Decision Records

Decisions that shape LexiSwitch, with the reasoning that produced them. Each record is
immutable once accepted — when a decision changes, write a new ADR that supersedes the
old one rather than editing history.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-windows-first-with-os-free-core.md) | Windows-first, with an OS-free core | Accepted |
| [0002](0002-rust-as-implementation-language.md) | Rust as the implementation language | Accepted |
| [0003](0003-raw-input-over-low-level-hook.md) | Raw Input observation instead of a low-level keyboard hook | Accepted |
| [0004](0004-runtime-derived-layout-tables.md) | Derive layout tables at runtime from installed layouts | Accepted |
| [0005](0005-bounded-in-memory-keystroke-buffer.md) | Bounded in-memory keystroke buffer, with containment rules | Accepted |
| [0006](0006-uia-first-clipboard-fallback.md) | UI Automation first, clipboard round-trip as fallback | Superseded in part by [0010](0010-paste-selection-replacement.md) |
| [0007](0007-manual-only-v1.md) | Manual hotkeys only in v1; automatic switching deferred | Accepted |
| [0008](0008-tray-and-toml-configuration.md) | Tray icon and TOML file for configuration; GUI deferred | Accepted |
| [0009](0009-multi-tap-layout-cycling.md) | Multi-tap cycling as target selection and undo | Accepted |
| [0010](0010-paste-selection-replacement.md) | Replace selections by pasting, not by synthesising keystrokes | Accepted |

## Format

```markdown
# [Title]

## Status
Proposed | Accepted | Deprecated | Superseded by [ADR-xxx]

## Context
The issue being addressed — background, constraints, goals, assumptions.

## Decision
What was decided.

## Consequences
Results of the decision, positive and negative.

## Alternatives Considered
What else was on the table, and why it lost.

## Related
Links to related ADRs and documentation.
```
