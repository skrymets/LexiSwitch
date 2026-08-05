# ADR-0004: Derive layout tables at runtime from installed layouts

## Status

Accepted

## Context

Converting `ауфегкуі` back to `lexiswitch` is not a character-to-character translation
between alphabets. It is a reinterpretation of *physical key positions* under a different
layout: the key that produces `а` on Ukrainian produces `f` on US English because it is
the same physical key.

So the conversion needs, per layout, a table mapping `(scancode, shift state) → character`
and its inverse. Two ways to obtain those tables:

- Hardcode them as data in the binary.
- Ask Windows at startup which layouts are installed and probe each one.

Hardcoding looks simpler until layout *variants* are considered. "English" is US, US
International, UK, Dvorak, and Colemak — all different. "German" is QWERTZ with `y` and
`z` transposed relative to QWERTY, plus a distinct punctuation arrangement and dead keys
for `^` and `´`. A hardcoded table that assumes the wrong variant produces confidently
wrong output, which is worse than an error.

Windows exposes what is actually installed: `GetKeyboardLayoutList` enumerates the layout
handles, and `ToUnicodeEx` translates a virtual key plus keyboard state under a specified
layout handle.

## Decision

Build all layout tables at startup by enumerating installed layouts with
`GetKeyboardLayoutList` and probing each with `ToUnicodeEx` across the alphanumeric and
punctuation scancode range, in unshifted and shifted states.

Layout tables are runtime data. **No layout data is compiled into the binary.**

Refuse to start when fewer than two layouts are installed, with a message explaining why.

## Consequences

**Positive.** The user's actual layout variants are used, whatever they are. Adding a
language is installing a Windows keyboard layout — a settings change, not a code change,
no release required. Uncommon and custom layouts work without anyone anticipating them.
No layout data to source, license, verify, or keep current.

**Negative.** More Win32 work at startup than reading a constant, and a class of failure
that constants do not have: if probing misbehaves, every conversion is wrong. This is why
`dump_layouts` exists as a verification example (see the spec) and why table extraction is
a Phase 0 spike.

**Dead keys are the sharp edge.** `ToUnicodeEx` maintains internal dead-key state. Probing
the German `^` key leaves that state pending, and subsequent probes return combined
characters instead of the expected ones — silently. Each probe must flush pending dead-key
state before proceeding. Keys that resolve to a dead key are marked as such in the table
and excluded from conversion rather than mapped to a plausible-looking wrong character.
This is the specific risk the Phase 0 spike is meant to expose.

**Testing needs frozen data anyway.** `lexiswitch-core` has no Win32 access by design
(ADR-0001), so its tests cannot call `ToUnicodeEx`. The `dump_layouts` example serves
double duty: it generates JSON fixtures for uk, en-US, and de, which are committed and
frozen. Core tests run against fixtures; the extraction path is verified by inspection.

## Alternatives Considered

**Hardcoded tables for uk / en-US / de.** Deterministic, trivially testable, no startup
cost. Rejected: it silently assumes layout variants. A user on US International or a
German QWERTZ that differs in punctuation gets wrong output with no indication anything
is off — and adding a fourth language becomes a code change.

**Hardcoded, uk ↔ en-US only.** The narrowest v1. Rejected for the same reason plus a
second: German was named in the original brief, so the narrower scope does not even cover
the stated need.

**Hybrid — runtime derivation with hardcoded fallback.** Rejected as the worst option:
carries both mechanisms and both failure modes, and the fallback is most likely to engage
in exactly the unusual configurations where hardcoded tables are least likely to be right.

## Related

- [ADR-0001](0001-windows-first-with-os-free-core.md) — why core cannot call Win32 directly
- [ADR-0003](0003-raw-input-over-low-level-hook.md) — Raw Input supplies the scancodes these tables are keyed on
- [ADR-0009](0009-multi-tap-layout-cycling.md) — choosing among three or more derived layouts
