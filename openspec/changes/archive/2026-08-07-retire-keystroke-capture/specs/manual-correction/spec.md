## REMOVED Requirements

### Requirement: Convert the last typed word

**Reason**: Replacing the most recently typed word requires knowing what was typed, which
requires observing typing. The observation subsystem is the largest and least auditable part
of the program, and this is the only feature that consumes it
([ADR-0011](../../../../docs/adr/0011-no-keyboard-observation.md)). The feature also carried
defects that were never repaid: it replaced text one character at a time, costing one undo
step per character and exposing every correction to the delivery loss recorded in
`docs/spikes/2026-08-06-selection-replacement-delivery.md`.

**Migration**: Select the text and use the selection hotkey. This is one extra action —
selecting — and in exchange the operation works on text typed at any time, in any application,
including text the program never saw being typed.

### Requirement: Convert the clipboard

**Reason**: Converting clipboard contents duplicates the selected-text use case while
expanding the product's surface. The clipboard remains an internal transport for reading and
replacing a selection; it stops being a user-facing target of its own.

**Migration**: Paste the text, select it, and convert the selection. The clipboard hotkey is
removed from configuration.

### Requirement: Repeated presses cycle through candidate layouts

**Reason**: Cycling depends on state the user cannot see, on timing, and on an
installed-layout order nobody chose. On a selection it cannot work at all: after a replacement
nothing is selected, so a second press finds no selection, falls through to the clipboard
read, and gets back clipboard contents it cannot distinguish from a real selection
([ADR-0012](../../../../docs/adr/0012-single-chooser-transaction.md)).

**Migration**: None available in this change — this removes cycling without yet providing the
chooser that replaces it. Until the chooser lands, one press converts using the direction the
system selects, and a conversion is undone with the host application's `Ctrl+Z`, which reverts
it in one step. The `multi_tap_window_ms` setting is removed.
