## Why

The shell around the conversion has two failure modes that make the product hard to recover
from, and both are one-line policy choices rather than missing machinery.

A hotkey that cannot be registered ends the program at startup. The user gets no window, no
tray icon, and no way to pick a different combination — the one situation where configuration
matters most is the one where configuration is unreachable. `Shift+Pause` is unclaimed on the
development machine, but nothing guarantees that anywhere else.

Fewer than two installed layouts also ends the program, for the same reason and with the same
result: a user who needs to be told what to install is told nothing, by an application that
has already exited.

Separately, the shell still describes a product that no longer exists. The tray offers to pause
keystroke capture that has been removed, and the configuration file carries settings whose
referents are gone.

## What Changes

- **BREAKING** A hotkey that cannot be registered no longer ends the program. The application
  stays running, shows the conflicting combination, and offers a keyboard-operable way to
  choose another. It never silently substitutes one.
- **BREAKING** Fewer than two installed layouts no longer ends the program. It starts in a
  disabled state that explains what is required, with settings and diagnostics reachable.
- Add shortcut rebinding: record a combination by pressing it, validate that it can be
  registered, apply it without restarting, and keep the previous working combination if the
  new one fails.
- Add distinguishable tray states — enabled, disabled, too few layouts, shortcut conflict,
  runtime error — with the active shortcut visible in the tooltip.
- Add a tray command that invokes a conversion, as a fallback path that does not depend on the
  global shortcut being registrable.
- Reduce configuration to the settings that remain and make the state that matters reachable
  from the tray rather than only from the file.

## Capabilities

### Modified Capabilities

- `agent-control`: hotkey conflicts become a recoverable state rather than a fatal one; tray
  controls gain the states and the fallback invocation; configuration narrows and becomes
  reachable through the interface as well as the file.
- `layout-conversion`: the minimum-two-layouts rule changes from refusing to start to starting
  disabled and explaining why.

### New Capabilities

None. This change reshapes the existing shell.

## Impact

**Smaller than it looks.** `Hotkeys::register` already returns a `Result`, `Drop` already
unregisters, and configuration reload already re-registers. What is missing is the policy at
startup — which currently propagates the error and exits — and the recording interface. The
runtime re-registration this change needs is already built and exercised.

**New user interface.** Recording a shortcut requires a focused window that reads key
combinations, which is the second small window in the product after the chooser and should
reuse whatever that establishes.

**Independent of the chooser.** Both this change and `add-candidate-chooser` depend on
`retire-keystroke-capture` for the removals; neither depends on the other. This one can land
first if the chooser proves slow.

**Verification is the point, not an afterthought.** The user-facing shell is the least verified
part of the current product — the tray, configuration reload, conflict reporting and failure
surfacing have never been exercised end to end. This change carries that debt.
