## Why

Conversion today guesses. It takes the direction from whichever input language the foreground
window happens to have active, which is not evidence of what the text was typed under — a user
who types Ukrainian under an English layout, switches away, and comes back finds the
conversion silently does nothing, because the window is now on English and Cyrillic maps to
nothing there.

It also decides alone. Text changes the moment the hotkey is pressed, so success, no-change
and failure all look the same from the outside, and the only way to see an alternative is to
press again within a timing window and hope.

Both problems have the same fix: generate every plausible conversion from the selected text
itself, show them, and change nothing until the user picks one. That is the pivot's central
interaction ([ADR-0012](../../../docs/adr/0012-single-chooser-transaction.md)).

The model was measured before being specified. A chooser must take focus to read the keyboard,
and the alternative that avoids taking focus needs the keyboard hook this project forbids — so
everything depends on the target's selection surviving the round trip. It does, across a Win32
control with a text pattern, a Scintilla control without one, and a Chromium text field
(`docs/spikes/2026-08-07-focus-round-trip.md`).

## What Changes

- Add a keyboard-driven candidate chooser, opened by the invocation hotkey, that shows every
  distinct candidate at once and modifies nothing until the user commits.
- **BREAKING** Stop deriving the conversion direction from the foreground window's active
  input language. Sources are inferred from the selected characters; every non-self pair of
  installed layouts is a candidate, and the active layout becomes contextual display and a
  weak ranking hint only.
- **BREAKING** Replace automatic post-conversion layout switching with an explicit
  `Ctrl+Enter`, which names the layout it will switch to. `Enter` applies without touching the
  active layout.
- Add candidate filtering and ranking: drop results identical to the original, deduplicate on
  the pair of visible result **and** target layout, and order by deterministic signals with no
  dictionary, model or data file.
- Add compact previews with a preview window that favours changed text, expandable for long
  selections.
- Add distinguishable outcomes for success, cancellation, no valid conversion, selection-read
  failure, replacement failure and layout-switch failure.
- Restore focus to the original application after every outcome.

## Capabilities

### New Capabilities

- `candidate-chooser`: the conversion transaction — opening on invocation, presenting
  candidates, keyboard navigation and previews, commit and cancel, focus restoration, and the
  outcome states the user can tell apart.

### Modified Capabilities

- `layout-conversion`: gains candidate generation. Conversion between two named layouts is
  unchanged; what is new is producing the *set* of plausible conversions from selected text,
  and the filtering, deduplication and ranking that reduce it to what is worth showing.
- `manual-correction`: converting a selection becomes chooser-mediated rather than immediate,
  and the active layout follows a correction only when the user asks for it.

`agent-control` is unchanged by this change; the shortcut and tray rework is separate.

## Impact

**New** — a chooser window and its transaction state in `lexiswitch-win`, and candidate
generation, filtering and ranking in `lexiswitch-core`. The generation logic is pure and
belongs in the core crate, so it is testable without Windows against the frozen fixtures.

**The chooser is the largest single item in the pivot.** This codebase has no user interface
beyond a 205-line tray menu, and the single-executable and 5 MB constraints rule out a UI
framework, so it is hand-built on Win32.

**Depends on `retire-keystroke-capture`** for the removal of cycling, whose repeated-press
semantics this replaces. It does not depend on it for the conversion engine, which is
untouched by either change.

**Reuses the replacement path unchanged.**
[ADR-0010](../../../docs/adr/0010-paste-selection-replacement.md) already delivers a
replacement that is one logical edit and one undo step; the pivot's §19.5 is satisfied before
this change starts.

**Read paths are chosen per invocation, never cached.** Chromium was observed exposing no text
pattern on one run and a working one on the next, so any per-application memory of which read
path works would be wrong about half the time.
