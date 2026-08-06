# ADR-0006: UI Automation first, clipboard round-trip as fallback

## Status

Superseded in part by [ADR-0010](0010-paste-selection-replacement.md)

The **read** strategy below stands unchanged: UI Automation `TextPattern` first, clipboard
round-trip as fallback. What ADR-0010 replaces is how the converted text is *written* back —
by pasting rather than by synthesising keystrokes — and with it the claim in "Consequences"
that the automation path never touches the clipboard. It now does.

## Context

Converting the last typed word needs no cooperation from the target application:
LexiSwitch knows what was typed from its own buffer, and replaces it with backspaces plus
Unicode injection.

Converting *highlighted* text is different. The selection lives inside another process,
and Windows offers no universal way to read it. Two mechanisms exist:

- **UI Automation `TextPattern`** — read and replace the selection through the
  accessibility layer, without touching the clipboard. Clean, but coverage depends
  entirely on the target application implementing the pattern. Many Electron, Chromium,
  Java, and terminal applications expose nothing usable.
- **Clipboard round-trip** — send `Ctrl+C`, read the clipboard, convert, set it, send
  `Ctrl+V`, restore the previous contents. Works nearly everywhere, because it uses the
  same path the user would. But it borrows a shared, observable global resource.

The original brief called for preserving clipboard contents, which rules out a naive
round-trip that leaves the converted text sitting in the clipboard afterward.

## Decision

Attempt **UI Automation `TextPattern`** first. Fall back to a **clipboard round-trip with
save and restore** when the application exposes no usable text pattern.

Clipboard handling specifics:

- Snapshot `CF_UNICODETEXT` before the operation and restore it after.
- Leave other formats untouched rather than attempt to reconstruct them. A partial,
  plausible-looking restoration of rich or image content is worse than an honest
  text-only snapshot.
- On failure to open the clipboard — another process holds it — retry three times with
  backoff, then **abort without pasting** and notify via the tray.

## Consequences

**Positive.** Selection conversion works across the full application matrix, not just
accessibility-cooperative apps. In applications that do support `TextPattern`, the
clipboard is never touched at all, which is both cleaner and faster. The UIA path is
tried first, so the common editors benefit from it.

**Negative.** Two code paths and the detection logic that chooses between them — more
surface than either alone. UIA also brings COM lifetime management into `lexiswitch-win`,
one of the few places where Rust's guarantees stop at the FFI boundary and correctness
depends on getting `unsafe` right.

**The clipboard fallback has a visible privacy edge.** For the duration of the round-trip,
both the original and converted text pass through the system clipboard, where clipboard
managers and Windows Clipboard History will record them. Restoration returns the previous
value but cannot un-record what a history tool captured. This is inherent to the mechanism
— it is the same exposure the user creates by pressing `Ctrl+C` themselves — and is
documented rather than mitigated. Users who consider it unacceptable can disable the
fallback in config, accepting that selection conversion then works only in UIA-capable
applications.

**Abort-without-paste is deliberate.** A half-completed clipboard operation that leaves
the user's clipboard destroyed is a worse outcome than a conversion that did not happen.
The failure path prioritizes not damaging state over completing the action.

## Alternatives Considered

**Clipboard round-trip only.** One path, universal coverage, straightforward. Rejected:
it forces the clipboard exposure even in applications where UIA would have avoided it
entirely, and clipboard-history capture is precisely the kind of incidental leakage this
project is meant to avoid where avoidable.

**UI Automation only.** Cleanest privacy story — the clipboard is never involved.
Rejected on coverage: a large share of conversions would silently fail in exactly the
Chromium and Electron applications where mistyped text most often appears. A feature that
works unpredictably is worse than one that works via a documented compromise.

## Related

- [ADR-0005](0005-bounded-in-memory-keystroke-buffer.md) — why last-word conversion needs none of this
- [ADR-0008](0008-tray-and-toml-configuration.md) — where the fallback is disabled
- [Spec: manual-correction](../../openspec/changes/add-manual-layout-conversion/specs/manual-correction/spec.md) — selection and clipboard failure behavior
