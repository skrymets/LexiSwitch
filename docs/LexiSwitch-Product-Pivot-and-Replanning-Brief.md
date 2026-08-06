# LexiSwitch Product Pivot and Replanning Brief

## 1. Document purpose

This document defines the approved product pivot for LexiSwitch.

It is intended as input for a planning agent that will:

- retire obsolete use cases and requirements;
- identify code and architecture that can be removed;
- rewrite affected specifications;
- prepare new implementation plans and tasks;
- preserve still-valid behaviour;
- add missing UX, reliability, privacy, and verification requirements.

This is a product and requirements baseline. It does not prescribe a particular Windows UI framework, programming language, accessibility API, clipboard implementation, or internal architecture unless that choice is necessary to enforce the required product behaviour.

---

## 2. Executive decision

LexiSwitch will no longer be treated as a background typing observer that reconstructs recently typed text.

It will become an explicitly invoked keyboard-layout reinterpretation utility for selected text.

The primary workflow is:

```text
Select text
    ↓
Invoke LexiSwitch
    ↓
See all distinct conversion candidates
    ↓
Select a candidate
    ├── Enter       → Apply conversion
    ├── Ctrl+Enter  → Apply conversion and switch layout
    └── Esc         → Cancel without modification
```

The application must not:

- monitor general keyboard input;
- maintain a history of typed keys;
- reconstruct the last typed word;
- automatically correct text while the user is typing;
- cycle candidates through repeated shortcut presses;
- depend on the current Windows layout as the historical source layout.

---

## 3. Terminology

LexiSwitch does not translate natural language.

It reinterprets characters according to the physical key positions of another installed keyboard layout.

The following terms must be used consistently:

- **Original text**: the text explicitly selected by the user.
- **Source layout**: the keyboard layout that could have produced the original characters.
- **Target layout**: the layout under which the same physical key positions should be interpreted.
- **Current Windows layout**: the layout currently active in the focused application.
- **Candidate**: one distinct non-self source-to-target reinterpretation of the original selection.
- **Conversion transaction**: the period from invocation until apply, apply-and-switch, cancel, or failure.

The current Windows layout is contextual information only. It is not authoritative evidence of the source layout.

---

## 4. Current-state diagnosis

The existing application is largely implemented according to its original requirements, but several original use cases produce an unsuitable product model.

The major causes of the current unusable state are:

1. Last-word correction requires continuous keyboard observation.
2. Continuous observation resembles keylogger behaviour to endpoint protection.
3. Last-word reconstruction creates privacy, security, focus, timeout, and buffer-management complexity.
4. Per-character text synthesis creates unreliable replacement and fragmented undo behaviour.
5. Repeated shortcut presses create hidden state and timing dependencies.
6. Candidate cycling depends on installed-layout ordering that the user must remember.
7. Selection loss after replacement makes repeated cycling unreliable.
8. The current foreground layout is incorrectly treated as the conversion source.
9. Successful, failed, and no-change operations are insufficiently distinguishable.
10. Automatic layout switching is an implicit side effect rather than an explicit user decision.
11. Multiple overlapping correction modes make the utility harder to understand and verify.
12. Tray-only feedback is too distant from the text correction transaction.

The core problem is therefore not a missing feature. The core problem is that the original requirements optimise for invisible automation instead of explicit, reliable control.

---

## 5. Intention to change

The redesign must prioritise:

- explicit user intent;
- deterministic behaviour;
- visible source and target layouts;
- minimal hidden state;
- keyboard-only operation;
- safe cancellation;
- reliable replacement;
- straightforward recovery;
- no background input monitoring;
- no requirement to remember layout order;
- no timing-sensitive multi-press interaction;
- no external dictionaries or language models;
- a single executable distribution.

The user should always be able to answer:

- What text is being processed?
- Which source layout is assumed?
- Which target layout will be used?
- What will the resulting text look like?
- Will the active Windows layout also change?
- Has the operation been applied, cancelled, or failed?

---

## 6. Recommended conceptual pivot

### Previous concept

> A background typing observer that can reconstruct and cycle recently typed text between keyboard layouts.

### Revised concept

> A keyboard-layout reinterpretation command for explicitly selected text.

### Product statement

LexiSwitch is a small Windows tray utility that converts explicitly selected text between installed keyboard layouts by reinterpreting physical key positions.

It presents all distinct valid conversion candidates in one keyboard-controlled chooser and modifies the selected text only after explicit confirmation.

---

## 7. Revised product boundary

### 7.1 In scope

- Windows 11.
- Two or more explicitly installed keyboard layouts.
- Explicitly selected text.
- Position-based keyboard-layout conversion.
- Source-to-target conversion between different installed layouts.
- Exclusion of self-to-self conversions.
- Generation of all distinct plausible candidates.
- Removal of unchanged candidates.
- Deduplication of identical candidate results.
- A single candidate chooser for every invocation.
- Deterministic candidate preselection.
- Keyboard-only candidate navigation.
- Explicit application with `Enter`.
- Explicit application and layout switching with `Ctrl+Enter`.
- Explicit cancellation with `Esc`.
- Compact previews for ordinary selections.
- Expandable previews for long selections.
- Whole-selection conversion.
- Unmappable characters passing through unchanged.
- Visible reporting of unchanged characters.
- Configurable global invocation shortcut.
- Explicit handling of shortcut registration conflicts.
- Tray-based status, configuration, diagnostics, and fallback invocation.
- Restoration of application focus after the transaction.
- One logical replacement operation where supported by the target application.
- No network access.
- No selected-text logging.
- No persistent storage of selected text.
- A single executable distribution.
- Runtime-generated user configuration where required.

### 7.2 Out of scope

- Last-word correction.
- Reconstruction of recently typed text.
- Continuous keyboard observation.
- Global keyboard hooks used to record ordinary typing.
- Automatic correction while typing.
- Automatic conversion without explicit invocation.
- Candidate cycling through repeated shortcut presses.
- Double-press timing windows.
- Hidden candidate indexes such as `1 of 2`.
- Dependence on installed-layout ordering.
- Treating the current Windows layout as the historical source.
- Dictionary-based language detection.
- Statistical language models.
- External language data files.
- External dictionaries.
- Online translation or network services.
- Suspicious-segment detection inside a mixed selection.
- Automatic conversion of only part of the selected text.
- First-class clipboard-content conversion.
- Per-application shortcut profiles.
- Dynamic shortcut reassignment based on the focused application.
- Modifier double-tap gestures.
- Leader-key sequences requiring general keyboard monitoring.
- Multiple variants of the same language beyond the layouts explicitly installed by the user.
- Automatic learning as a requirement for the first redesigned version.
- Near-caret positioning as a requirement for the first redesigned version.

---

## 8. Final interaction model

### 8.1 Invocation

The user:

1. selects text in the current application;
2. invokes LexiSwitch using the configured global shortcut.

A tray command may provide fallback invocation and diagnostics, but no required workflow step may depend on mouse input.

### 8.2 Candidate chooser

LexiSwitch opens one keyboard-controlled command-palette-style chooser on the active monitor.

The chooser must display:

- a compact preview of the original selected text;
- the current Windows layout as separate contextual information;
- every distinct valid candidate;
- source and target layouts for every candidate;
- a compact preview of each candidate result;
- the number of unchanged characters when relevant;
- the currently selected candidate;
- the available commit and cancellation actions.

Example:

```text
┌─ Correct selected text ───────────────────────────────────────┐
│ Current Windows layout: ENG                                  │
│ Original: “Ьн ігзук еуче шт Утпдшір”                          │
│                                                              │
│ › УКР → ENG  “My super text in English”                       │
│   УКР → DEU  “Mz super text in English”                       │
│                                                              │
│ ↑↓ Select                 Space Expand preview                │
│ Enter Apply               Ctrl+Enter Apply + switch to ENG    │
│ Esc Cancel                                                   │
└──────────────────────────────────────────────────────────────┘
```

There must not be separate “normal” and “ambiguous” interfaces.

The same chooser is used when there is:

- one candidate;
- two candidates;
- several candidates;
- high confidence;
- low confidence.

Confidence affects only candidate ordering and preselection. It does not change the basic interaction model.

### 8.3 Candidate visibility

All distinct candidates must be visible in one list when they fit.

Candidate browsing must not use pagination semantics such as:

```text
1 of 2
2 of 2
```

The user must not repeatedly invoke the global shortcut to reveal another candidate.

For unusually large candidate sets, the list may scroll, but it remains one chooser and one transaction.

### 8.4 Candidate selection

- `Up` and `Down` change the selected candidate.
- The most plausible candidate is preselected.
- Selection does not modify the original text.
- Candidate navigation always uses the original selection as input.
- Candidate navigation does not chain conversions through previous candidate results.

### 8.5 Apply

Pressing `Enter`:

1. applies the selected candidate;
2. does not intentionally change the current Windows layout;
3. restores focus to the original application;
4. closes the conversion transaction;
5. displays a concise success indication when necessary.

### 8.6 Apply and switch layout

Pressing `Ctrl+Enter`:

1. applies the selected candidate;
2. requests activation of the candidate's target layout;
3. restores focus to the original application;
4. closes the conversion transaction;
5. reports that layout switching was requested;
6. reports failure if the target application rejects or does not support the switch, where this can be detected.

The chooser must make the target explicit:

```text
Ctrl+Enter Apply + switch to ENG
```

Automatic layout switching must not occur as an invisible default side effect.

### 8.7 Cancel

Pressing `Esc`:

1. closes the chooser;
2. leaves the selected text unchanged;
3. restores the original application focus;
4. restores any temporarily modified clipboard state where applicable;
5. discards the transaction.

### 8.8 High-confidence behaviour

The redesigned baseline must not immediately modify text on invocation.

Even when there is one candidate or one highly ranked candidate:

- the chooser opens;
- the candidate is preselected;
- `Enter` applies it.

This provides one consistent and trustworthy workflow.

Adaptive immediate application may be considered as a future optional feature, but it is not part of the initial redesigned product.

---

## 9. Candidate generation rules

For `N` installed layouts, the theoretical source-to-target set contains:

```text
N × (N - 1)
```

ordered pairs.

For three installed layouts:

```text
ENG → DEU
ENG → УКР
DEU → ENG
DEU → УКР
УКР → ENG
УКР → DEU
```

The following pairs are prohibited:

```text
ENG → ENG
DEU → DEU
УКР → УКР
```

Candidate generation must follow this sequence:

1. Read the original selected text.
2. Enumerate installed source layouts.
3. Determine whether each source layout can plausibly represent the relevant characters.
4. Enumerate every other installed layout as a target.
5. Reinterpret physical key positions from source to target.
6. Preserve shift state and case where mappings permit it.
7. Pass unmappable characters through unchanged.
8. Remove results identical to the original selection.
9. Deduplicate identical visible results.
10. Rank the remaining candidates.
11. Preselect the highest-ranked candidate.
12. Show all distinct candidates in the chooser.

All candidates must be generated from the original selected text.

---

## 10. Candidate ranking

Candidate ranking must remain lightweight and standalone.

It must not require:

- dictionaries;
- natural-language models;
- network access;
- downloaded data;
- files shipped alongside the executable.

Permitted deterministic ranking signals include:

1. Whether the original characters can be represented by the proposed source layout.
2. The proportion of characters that can be mapped.
3. The number of characters that remain unchanged.
4. Whether the conversion actually changes visible text.
5. Whether the result introduces control characters or invalid output.
6. Whether the source character set uniquely indicates one installed layout.
7. Whether multiple conversion paths produce the same visible result.
8. The current Windows layout as a weak contextual hint only.
9. A small, locally stored preference for recently chosen layout pairs, if added later.

The chooser must not present linguistic certainty that the application does not possess.

Preferred labels are:

- **Detected** when the source or result is mechanically unique.
- **Suggested** when deterministic heuristics produced a ranking.
- **Selected** when the user explicitly chose the candidate.

Automatic learning from previous choices is not required for the first redesigned version.

If later added, learning must:

- remain local;
- remain resettable;
- not require additional distribution files;
- not hide candidates;
- not make behaviour unpredictable;
- affect ordering only.

---

## 11. Long-text preview design

The selected text may be longer than can be usefully displayed in every candidate row.

The chooser must therefore use progressive disclosure.

### 11.1 Compact candidate row

Each candidate row should show:

- source and target;
- one or two lines of converted text;
- ellipsis when content is omitted;
- total character count where useful;
- unchanged-character count where relevant.

Example:

```text
› УКР → ENG  “This is the beginning of the corrected sentence…”
             184 characters · 3 unchanged
```

### 11.2 Useful preview window

The compact preview should prefer a region containing meaningful changed text.

It should not always display only the first characters when the beginning contains unchanged material such as:

- URLs;
- identifiers;
- numbers;
- code prefixes;
- email addresses.

Example:

```text
Original:
https://example.com — Ьн ігзук еуче шт Утпдшір

Compact candidate preview:
…example.com — My super text in English
```

### 11.3 Expanded preview

Pressing `Space` expands or collapses the currently selected candidate preview.

The expanded preview must:

- wrap long lines;
- avoid horizontal scrolling where possible;
- support keyboard scrolling;
- clearly distinguish original and candidate text;
- remain inside the same transaction.

Suggested controls:

```text
Space       Expand or collapse preview
PageUp      Scroll expanded preview upward
PageDown    Scroll expanded preview downward
Home        Move to preview beginning
End         Move to preview end
```

### 11.4 Preview placement

The initial redesigned version should use a stable command palette on the active monitor.

Near-caret placement is deferred because it creates additional complexity involving:

- caret detection;
- selection geometry;
- screen-edge repositioning;
- overlays covering selected text;
- compatibility across different applications;
- long-preview layout.

A future near-caret compact mode may be considered after the stable palette is proven reliable.

---

## 12. Mixed selections

The application converts the exact text selected by the user.

It does not attempt to identify or correct only suspicious fragments.

For mixed selections containing:

- multiple languages;
- URLs;
- identifiers;
- code;
- email addresses;
- numbers;
- punctuation;

the initial product converts all mappable characters and passes unmappable characters through unchanged.

The chooser must disclose partial conversion where relevant:

```text
3 characters unchanged
```

Automatic suspicious-segment detection is out of scope because it would introduce additional inference, hidden scope decisions, and possible damage to correctly typed content.

The selected range is the user's explicit declaration of scope.

---

## 13. Shortcut strategy

### 13.1 Global invocation shortcut

LexiSwitch must use one configurable registered global shortcut for invocation.

The default may remain `Shift+Pause`, but the product must not assume that this combination is universally available.

### 13.2 Conflict detection

If the configured shortcut cannot be registered:

- the application must not silently choose another shortcut;
- the application must not terminate without explanation;
- the application must remain available in the tray;
- the conflicting combination must be displayed;
- a keyboard-operable conflict dialog must be shown;
- the user must be able to assign another shortcut;
- the error state must remain discoverable until resolved.

Example:

```text
Shortcut unavailable: Shift+Pause

The shortcut is already registered by another application.

Press a new shortcut combination, or press Esc to keep LexiSwitch disabled.
```

### 13.3 Rebinding

Shortcut configuration should support:

- recording a new combination by pressing it;
- validating whether it can be registered;
- applying the new combination without restarting;
- restoring the previous valid combination if registration fails;
- showing the active shortcut in the tray tooltip and settings.

### 13.4 Suggested alternatives

The configuration UI may suggest combinations such as:

```text
Ctrl+Win+Pause
Ctrl+Alt+Pause
Ctrl+Win+Shift+Space
```

These are suggestions only. None is guaranteed to be conflict-free.

### 13.5 Prohibited shortcut approaches

The initial redesign must not use:

- per-application dynamic shortcuts;
- silent fallback shortcuts;
- modifier double taps;
- long-press detection;
- leader-key sequences;
- timing-sensitive shortcut sequences;
- general keyboard observation to detect invocation.

### 13.6 Local chooser shortcuts

Only the initial invocation shortcut must be global.

After the chooser opens, all controls are local to LexiSwitch:

- arrows;
- `Space`;
- `Enter`;
- `Ctrl+Enter`;
- `Esc`;
- preview scrolling keys.

This minimises the global shortcut surface and reduces collision risk.

---

## 14. Tray role

The tray is a status and configuration surface, not the primary conversion chooser.

### 14.1 Tray responsibilities

The tray should provide:

- current enabled or disabled status;
- shortcut conflict status;
- runtime error status;
- active shortcut in the tooltip;
- settings;
- enable or disable invocation;
- reload configuration where still supported;
- fallback “Correct selected text” command;
- diagnostics;
- exit.

### 14.2 Tray status

The tray icon or tooltip should distinguish at least:

```text
LexiSwitch — Enabled — Shift+Pause
LexiSwitch — Disabled
LexiSwitch — Shortcut conflict: Shift+Pause
LexiSwitch — Error: selection unavailable
```

### 14.3 Source and target display

The tray does not need to permanently display source and target layouts because no conversion transaction exists while the application is idle.

During a conversion transaction, source and target must be shown in the chooser.

### 14.4 Pause terminology

The previous pause/resume concept was tied to continuous keyboard observation.

It should be replaced with:

```text
Enable LexiSwitch
Disable LexiSwitch
```

Disabling LexiSwitch means that the invocation shortcut is inactive or ignored. It does not refer to pausing keyboard capture because keyboard capture no longer exists.

---

## 15. Revised core product use cases

### UC-01: Start with two or more installed layouts

**Preconditions**

- Windows 11 is running.
- Two or more keyboard layouts are installed.

**Main flow**

1. LexiSwitch starts.
2. It discovers the installed layouts.
3. It builds runtime conversion tables.
4. It registers the configured global shortcut.
5. It enters the enabled tray state.

**Postconditions**

- The utility is ready to process selected text.
- No ordinary keyboard input is monitored.

---

### UC-02: Start with fewer than two installed layouts

**Preconditions**

- Fewer than two keyboard layouts are installed.

**Main flow**

1. LexiSwitch starts.
2. It determines that conversion is unavailable.
3. It remains running in a disabled tray state.
4. It explains that at least two installed layouts are required.

**Postconditions**

- The application does not crash or exit without explanation.
- Settings and diagnostics remain available.

---

### UC-03: Correct selected text

**Preconditions**

- At least two layouts are installed.
- LexiSwitch is enabled.
- The user has selected text in another application.

**Main flow**

1. The user invokes LexiSwitch.
2. LexiSwitch reads the selected text.
3. It snapshots the original text and required transaction state.
4. It generates all non-self candidates.
5. It removes unchanged and duplicate candidates.
6. It ranks the remaining candidates.
7. It opens the chooser.
8. It preselects the highest-ranked candidate.
9. The user reviews the candidates.
10. The user presses `Enter`.
11. LexiSwitch replaces the selected text.
12. LexiSwitch restores focus to the original application.
13. The transaction closes.

**Postconditions**

- The selected text contains the chosen result.
- The active Windows layout is not intentionally changed.
- No selected text is retained by LexiSwitch.

---

### UC-04: Correct selected text and switch layout

**Preconditions**

- The chooser is open.
- A candidate is selected.

**Main flow**

1. The user presses `Ctrl+Enter`.
2. LexiSwitch replaces the selected text.
3. LexiSwitch requests activation of the candidate's target layout.
4. LexiSwitch restores focus to the original application.
5. LexiSwitch reports success or partial success.
6. The transaction closes.

**Postconditions**

- The selected text contains the chosen result.
- The target layout has been requested.
- Any detectable failure to switch is reported.

---

### UC-05: Review a long conversion candidate

**Preconditions**

- The chooser is open.
- A candidate preview is truncated.

**Main flow**

1. The user selects the candidate.
2. The user presses `Space`.
3. LexiSwitch expands the preview.
4. The user scrolls using keyboard controls.
5. The user presses `Space` again to collapse it, or applies/cancels directly.

**Postconditions**

- The original text remains unmodified until commit.

---

### UC-06: Cancel correction

**Preconditions**

- The chooser is open.

**Main flow**

1. The user presses `Esc`.
2. LexiSwitch closes the chooser.
3. LexiSwitch restores focus.
4. LexiSwitch restores temporary clipboard state where applicable.
5. LexiSwitch discards the transaction.

**Postconditions**

- The original selected text is unchanged.
- No candidate is applied.
- No transaction data is retained.

---

### UC-07: No valid candidate exists

**Preconditions**

- Text is selected.
- LexiSwitch is invoked.

**Main flow**

1. LexiSwitch generates candidates.
2. All results are unchanged, invalid, or duplicates of the original.
3. LexiSwitch displays a clear no-conversion state.

Example:

```text
No distinct keyboard-layout conversion was found for the selected text.
```

**Postconditions**

- The original text remains unchanged.
- The condition is distinguishable from application failure.

---

### UC-08: Selection cannot be read

**Preconditions**

- LexiSwitch is invoked.
- The focused application does not expose a readable selection, or selection acquisition fails.

**Main flow**

1. LexiSwitch attempts to read the selection.
2. The attempt fails.
3. LexiSwitch reports the reason where known.
4. No text replacement is attempted.

Example:

```text
Selected text could not be read from this application.
```

**Postconditions**

- The target application remains unchanged.
- The error is visible and diagnosable.

---

### UC-09: Replacement fails

**Preconditions**

- A candidate has been committed.
- The target application does not accept replacement.

**Main flow**

1. LexiSwitch attempts replacement.
2. Replacement fails or cannot be verified.
3. LexiSwitch attempts to preserve or restore the original state.
4. LexiSwitch reports failure.
5. The transaction closes safely.

**Postconditions**

- LexiSwitch does not report unconditional success.
- Original text and clipboard state are restored where possible.

---

### UC-10: Resolve shortcut conflict

**Preconditions**

- The configured shortcut cannot be registered.

**Main flow**

1. LexiSwitch remains running.
2. It enters a shortcut-conflict state.
3. It displays the unavailable combination.
4. The user opens the keyboard-operable shortcut editor.
5. The user records a new combination.
6. LexiSwitch validates and registers it.
7. The conflict state clears.

**Postconditions**

- The new shortcut is active.
- No restart is required.

---

### UC-11: Disable and re-enable LexiSwitch

**Main flow**

1. The user disables LexiSwitch from the tray or settings.
2. Invocation is disabled.
3. The user later enables LexiSwitch.
4. The configured shortcut becomes active again.

**Postconditions**

- No keyboard capture starts or stops because no general keyboard capture exists.

---

## 16. Feature-level change intentions

| Feature or behaviour                    | Why it is problematic                                                                 | Decision and intention to change                                                  |
|-----------------------------------------|----------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------|
| Last-word correction                    | Requires typing history, reconstruction, focus tracking, timeouts and fragile editing. | Remove the feature entirely.                                                      |
| Continuous keystroke observation        | Resembles keylogger behaviour and is blocked by endpoint protection.                   | Remove the complete observation and rolling-buffer subsystem.                     |
| Raw Input typing buffer                 | Exists only to support last-word reconstruction.                                       | Remove the buffer, page-locking, wiping and related lifecycle rules.              |
| Password-field detection                | Exists to make continuous capture safer but cannot remove the EDR concern.              | Remove it with the capture subsystem.                                             |
| Excluded-application capture rules      | Add configuration and verification complexity for an unwanted feature.                 | Remove capture-related exclusions.                                                |
| Per-character replacement               | Can lose characters, create redundant symbols and produce multiple undo operations.    | Replace selected text as one logical operation where supported.                   |
| Repeated shortcut cycling               | Hidden state, timing dependence, selection loss and unclear ordering.                  | Remove cycling. Show all candidates in one chooser.                               |
| Double-press timing window              | Creates race conditions and unpredictable interaction.                                | Remove all multi-press timing semantics.                                          |
| Installed-layout order as navigation    | Forces the user to remember an invisible system order.                                 | Present candidates by explicit source and target labels.                          |
| Current layout as conversion source     | Current state does not identify the historical source of selected text.                | Generate source candidates from selected characters and installed layouts.        |
| Immediate silent replacement            | Makes success, no-change and failure difficult to distinguish.                         | Require explicit `Enter` or `Ctrl+Enter` commit in the redesigned baseline.        |
| Separate normal and ambiguous UIs       | Creates unnecessary branching and inconsistent interaction.                           | Use one chooser for all candidate counts and confidence levels.                   |
| Paginated candidate preview             | Requires redundant actions and hides available alternatives.                          | Show all distinct candidates in one list.                                         |
| Full text in every candidate row        | Becomes unusable for long selections and duplicates content.                           | Use compact previews with optional expansion.                                     |
| Near-caret overlay as default           | Complicates placement, long previews and application compatibility.                    | Use a stable active-monitor command palette first.                                |
| Automatic layout switch after apply     | Changes future typing state as an implicit side effect.                                | Use explicit `Ctrl+Enter` for apply-and-switch.                                   |
| Tray-only feedback                      | Too far from the correction and insufficient for candidate comparison.                 | Put transaction feedback in the chooser; keep persistent state in the tray.       |
| Tray pause/resume                       | Refers to a capture model that will no longer exist.                                   | Rename to enable/disable LexiSwitch.                                               |
| Fixed global shortcut                   | May collide with IDEs and other applications.                                          | Make the shortcut configurable and validate registration.                         |
| Silent shortcut fallback                | Makes invocation unpredictable.                                                        | Prohibit silent reassignment; require explicit user selection.                    |
| Application exit on shortcut conflict   | Makes configuration and recovery difficult.                                            | Keep the app running in a visible conflict state.                                 |
| Dynamic per-application shortcuts       | Increase state, configuration and cognitive load.                                      | Exclude from the initial redesign.                                                 |
| Modifier double-tap invocation          | Usually requires broad keyboard observation.                                           | Exclude to avoid recreating capture and EDR risks.                                |
| First-class clipboard conversion        | Duplicates the selected-text use case and expands product scope.                        | Remove from the core product. Clipboard may remain an internal transport fallback.|
| Dictionary-based ranking                | Requires additional assets and introduces linguistic complexity.                       | Use deterministic layout and character heuristics only.                           |
| External language files                 | Violate the desired single-executable distribution.                                    | Do not ship dictionaries, models or sidecar language files.                       |
| Automatic learning                      | Can make ordering less predictable and requires additional state.                      | Defer. Permit only lightweight, local, resettable ordering preferences later.     |
| Suspicious-segment conversion           | Allows the app to redefine the user's selected scope.                                  | Convert the whole explicit selection only.                                        |
| Unmappable-character passthrough         | Can create partially converted results without the user noticing.                      | Keep passthrough, but report unchanged-character counts.                          |
| Silent no-change result                 | Looks identical to a broken invocation.                                                | Show a distinct “no conversion found” result.                                     |
| Refusal to start with one layout         | Prevents access to settings and diagnostics.                                            | Start disabled and explain that two layouts are required.                         |
| Selected-text persistence               | Creates privacy risk and is unnecessary.                                                | Prohibit persistence and text logging.                                            |
| Network capability                      | Unnecessary for deterministic layout conversion.                                       | Continue to prohibit network access.                                              |

---

## 17. Existing requirement disposition

The following table maps the original 28 requirements to the revised product direction.

| #  | Existing requirement summary                                      | Disposition | Revised direction                                                                 |
|----|-------------------------------------------------------------------|-------------|-----------------------------------------------------------------------------------|
| 1  | Tray icon is always shown                                         | Alter       | Keep tray presence; add enabled, disabled, conflict and error states.             |
| 2  | Tray provides pause, reload, config and quit                      | Alter       | Replace pause with enable/disable; add fallback invocation and diagnostics.       |
| 3  | One human-readable config describes all behaviour                 | Alter       | Keep readable storage, but configuration must also be discoverable through UI.    |
| 4  | Config changes apply without restart                             | Keep        | Continue supporting runtime application of valid settings.                        |
| 5  | Invalid config preserves the last known good state                | Keep        | Continue preserving the active valid configuration and reporting errors.          |
| 6  | Password-field detection cannot be disabled                      | Drop        | Password-field capture protection is unnecessary after capture removal.           |
| 7  | Hotkey conflict is reported and names the combination            | Alter       | Remain running, show conflict state and provide keyboard-operable rebinding.       |
| 8  | Every action-preventing failure reaches the user through tray     | Alter       | Transaction failures appear in the chooser; persistent failures appear in tray.   |
| 9  | No network capability                                            | Keep        | Continue enforcing and verifying no network access.                               |
| 10 | Logging is off by default and never records text                  | Strengthen  | Extend prohibition to original text, candidates, clipboard content and previews.  |
| 11 | Observation cannot alter input and uses no keyboard hook          | Replace     | Prohibit general keyboard observation entirely.                                   |
| 12 | At most 64 keystrokes are buffered in protected memory           | Drop        | Remove the keystroke buffer and all associated memory handling.                    |
| 13 | Keystroke-derived data never reaches disk                        | Replace     | No keystrokes are captured; selected text and candidates must never persist.       |
| 14 | Buffer disabled in passwords, exclusions and paused state         | Drop        | Remove all capture-state and excluded-application buffer logic.                    |
| 15 | Buffer wiped on focus change, delimiters, timeout and conversion  | Drop        | Remove buffer lifecycle behaviour.                                                 |
| 16 | Buffer stores physical key positions instead of characters       | Drop        | Historical typing reconstruction is no longer part of the product.                |
| 17 | Layout tables are built from installed layouts                   | Keep        | Continue building runtime tables without compiled-in layout data.                  |
| 18 | Application refuses to start with fewer than two layouts         | Alter       | Start in a disabled explanatory state while keeping settings accessible.           |
| 19 | Conversion is position-based and preserves shift and case        | Keep        | Preserve this as the core conversion rule.                                         |
| 20 | Unmappable and dead-key characters pass through unchanged        | Alter       | Keep passthrough and display unchanged-character counts.                           |
| 21 | No-change conversion is reported                                 | Strengthen  | Remove unchanged candidates and show a clear no-valid-conversion state.            |
| 22 | Round trip returns original text for mappable characters         | Keep        | Retain as a conversion-engine invariant and automated test.                        |
| 23 | Conversion occurs only through explicit hotkey                   | Strengthen  | Require explicit invocation and explicit commit; never correct automatically.      |
| 24 | Last-word hotkey replaces the most recently typed word           | Drop        | Remove the use case and all supporting capture and reconstruction code.            |
| 25 | Selection hotkey replaces highlighted text in place              | Alter       | Open chooser first; replace only after `Enter` or `Ctrl+Enter`.                    |
| 26 | Clipboard hotkey converts clipboard contents                     | Drop        | Remove as a first-class user feature.                                              |
| 27 | Repeated presses cycle candidate layouts from the original       | Drop        | Replace with one chooser showing all distinct candidates simultaneously.           |
| 28 | Active layout follows correction by default                      | Alter       | Make layout switching explicit through `Ctrl+Enter`.                               |

---

## 18. New target requirements

### 18.1 Product scope requirements

**TR-001**

LexiSwitch shall process text only when explicitly invoked by the user.

**TR-002**

LexiSwitch shall process only text explicitly selected by the user.

**TR-003**

LexiSwitch shall not monitor, record, reconstruct, or retain ordinary keyboard input.

**TR-004**

LexiSwitch shall not provide last-word correction.

**TR-005**

LexiSwitch shall not automatically modify text while the user is typing.

---

### 18.2 Layout discovery and conversion requirements

**TR-006**

LexiSwitch shall discover the keyboard layouts currently installed in Windows.

**TR-007**

LexiSwitch shall build conversion mappings at runtime from installed layouts.

**TR-008**

LexiSwitch shall not require compiled-in language dictionaries or external layout files.

**TR-009**

LexiSwitch shall generate only source-to-target pairs where source and target differ.

**TR-010**

LexiSwitch shall generate every valid non-self candidate from the original selected text.

**TR-011**

LexiSwitch shall remove candidates that do not change the original text.

**TR-012**

LexiSwitch shall deduplicate candidates with identical visible output.

**TR-013**

LexiSwitch shall preserve shift state and case where the layout mapping permits it.

**TR-014**

LexiSwitch shall pass unmappable characters through unchanged.

**TR-015**

LexiSwitch shall report the number of unchanged characters when a candidate is partially converted.

**TR-016**

Every candidate shall be generated independently from the original selected text.

---

### 18.3 Candidate chooser requirements

**TR-017**

Every invocation with at least one candidate shall open the same candidate chooser.

**TR-018**

The chooser shall show all distinct candidates in one list.

**TR-019**

The chooser shall not use repeated global shortcut presses to navigate candidates.

**TR-020**

The chooser shall display the source and target layout for every candidate.

**TR-021**

The chooser shall display the current Windows layout separately from the candidate source layout.

**TR-022**

The chooser shall display a compact preview of the original text.

**TR-023**

The chooser shall display a compact preview of every candidate.

**TR-024**

The chooser shall preselect the highest-ranked candidate.

**TR-025**

The chooser shall support candidate selection using keyboard arrow keys.

**TR-026**

The chooser shall support expandable and collapsible candidate previews.

**TR-027**

The chooser shall remain fully operable without a mouse.

---

### 18.4 Commit and cancellation requirements

**TR-028**

Pressing `Enter` shall apply the selected conversion without intentionally changing the active layout.

**TR-029**

Pressing `Ctrl+Enter` shall apply the selected conversion and request activation of the target layout.

**TR-030**

The chooser shall clearly display the target layout affected by `Ctrl+Enter`.

**TR-031**

Pressing `Esc` shall cancel the transaction without modifying the selected text.

**TR-032**

LexiSwitch shall not modify the selected text before explicit commit.

**TR-033**

Replacement shall be performed as one logical editing operation where supported by the target application.

**TR-034**

LexiSwitch shall restore focus to the original application after apply, apply-and-switch, cancel, or failure.

---

### 18.5 Shortcut requirements

**TR-035**

LexiSwitch shall support one configurable global invocation shortcut.

**TR-036**

LexiSwitch shall detect whether the configured shortcut can be registered.

**TR-037**

LexiSwitch shall not silently substitute another shortcut when registration fails.

**TR-038**

LexiSwitch shall remain running when shortcut registration fails.

**TR-039**

LexiSwitch shall show the conflicting shortcut combination.

**TR-040**

LexiSwitch shall provide keyboard-operable shortcut reassignment.

**TR-041**

A valid shortcut change shall apply without restarting LexiSwitch.

**TR-042**

Failure to register a new shortcut shall preserve the previous valid shortcut where possible.

---

### 18.6 Feedback and error requirements

**TR-043**

LexiSwitch shall distinguish success, cancellation, no valid conversion, selection-read failure, replacement failure, and layout-switch failure.

**TR-044**

LexiSwitch shall not report success when replacement has failed or cannot reasonably be assumed to have succeeded.

**TR-045**

When no valid conversion exists, LexiSwitch shall show a clear no-conversion result.

**TR-046**

When selected text cannot be read, LexiSwitch shall not attempt replacement.

**TR-047**

When replacement fails, LexiSwitch shall preserve or restore the original state where possible.

**TR-048**

Action-specific feedback shall be shown in or near the chooser transaction.

**TR-049**

Persistent application-state errors shall be reflected in the tray.

---

### 18.7 Privacy and distribution requirements

**TR-050**

LexiSwitch shall not log original selected text.

**TR-051**

LexiSwitch shall not log candidate text.

**TR-052**

LexiSwitch shall not persist selected text or candidate text.

**TR-053**

LexiSwitch shall not provide network capability.

**TR-054**

LexiSwitch shall be distributed as one executable without required language-model, dictionary, or layout sidecar files.

**TR-055**

Runtime-generated configuration may be stored in the user's application-data location.

**TR-056**

If the clipboard is used as an internal transport mechanism, LexiSwitch shall minimise the exposure period and restore the previous clipboard state where possible.

**TR-057**

LexiSwitch shall not claim that clipboard-derived text can never enter Windows Clipboard History unless this is structurally guaranteed by the selected implementation.

---

## 19. Planning implications

The replanning effort should be divided into the following workstreams.

### 19.1 Decommission keystroke capture

Remove or retire:

- Raw Input registration used for ordinary typing observation;
- rolling keystroke buffers;
- memory locking for captured keystrokes;
- buffer wiping rules;
- focus-change buffer handling;
- idle timeout handling;
- Enter, Tab, and Escape buffer delimiters;
- password-field detection used for capture;
- excluded-application capture configuration;
- pause/resume capture state;
- last-word reconstruction;
- last-word conversion hotkeys;
- capture-specific tests and documentation.

The planning agent must verify that removing these components does not leave dormant background input-observation capability in the release binary.

### 19.2 Remove cycling behaviour

Remove or retire:

- repeated-press timing windows;
- candidate indexes stored across invocations;
- original-text retention used only for cycling;
- layout-order navigation;
- second-press and third-press shortcut semantics;
- cycling-specific clipboard fallback behaviour;
- cycling-specific tests and documentation.

Replace them with one transaction-local candidate list.

### 19.3 Introduce the chooser transaction

Plan:

- selected-text acquisition;
- original-text snapshot;
- candidate generation;
- filtering and deduplication;
- candidate ranking;
- command-palette UI;
- keyboard navigation;
- compact previews;
- expanded previews;
- apply;
- apply-and-switch;
- cancel;
- focus restoration;
- error presentation.

### 19.4 Refactor conversion direction

Remove the assumption that the current Windows layout is the conversion source.

Plan candidate generation from:

- selected characters;
- installed layout mappings;
- valid non-self layout pairs.

Keep the current Windows layout only as:

- contextual display;
- optional weak ranking input;
- the layout that may be changed after `Ctrl+Enter`.

### 19.5 Replace text delivery

Retire per-character synthesis for correction operations.

Plan a replacement path that:

- works with selected text;
- minimises symbol loss;
- minimises duplicate symbols;
- avoids timing-sensitive `SendKeys` behaviour;
- produces one undo operation where supported;
- preserves clipboard contents where possible;
- reports failure accurately.

### 19.6 Rework layout switching

Replace automatic post-correction switching with explicit transaction commands:

```text
Enter       Apply
Ctrl+Enter  Apply and switch to target layout
```

Plan separate result states for:

- replacement succeeded;
- replacement and layout switch succeeded;
- replacement succeeded but layout switch failed or could not be confirmed.

### 19.7 Rework shortcut management

Plan:

- configurable global shortcut;
- startup registration;
- runtime re-registration;
- conflict detection;
- non-fatal conflict state;
- keyboard-operable shortcut recording;
- tray status;
- no silent fallback.

### 19.8 Rework tray behaviour

Replace capture-oriented controls with product-oriented controls:

```text
Correct selected text
Enable LexiSwitch
Disable LexiSwitch
Settings
Reload configuration
Diagnostics
Exit
```

Plan explicit states for:

- enabled;
- disabled;
- fewer than two layouts;
- shortcut conflict;
- runtime error.

### 19.9 Rework configuration

Remove obsolete settings for:

- capture buffer size;
- capture timeout;
- password detection;
- excluded capture applications;
- last-word hotkey;
- clipboard conversion hotkey;
- candidate cycling timeout;
- automatic layout switching as an unconditional default.

Add or retain settings for:

- invocation shortcut;
- enabled or disabled state;
- preview size;
- optional ranking preferences;
- logging enablement without text content;
- configuration reload.

A migration strategy must handle existing configuration files without crashing.

Obsolete options should be:

- ignored with a warning;
- removed through an explicit migration;
- or rejected with a precise explanation.

The chosen behaviour must be documented and tested.

### 19.10 Verification and end-to-end testing

The previous implementation had substantial verification gaps in the user-facing shell.

The new plan must include end-to-end verification for:

- tray startup;
- tray commands;
- shortcut registration;
- shortcut conflict handling;
- settings updates;
- configuration reload;
- selection reading;
- selection replacement;
- one-step undo where supported;
- focus restoration;
- candidate chooser navigation;
- long-preview expansion;
- `Enter`;
- `Ctrl+Enter`;
- `Esc`;
- no-candidate handling;
- application incompatibility;
- clipboard restoration;
- CrowdStrike or comparable endpoint-protection behaviour;
- absence of general keyboard capture;
- release-build behaviour, not only debug-build behaviour.

---

## 20. Compatibility and reliability principles

The implementation must assume that Windows applications differ in how they expose and replace selected text.

The plan should define:

1. A preferred selection-reading and replacement path.
2. One or more safe fallback paths.
3. A clear unsupported-application result.
4. Clipboard preservation behaviour.
5. Focus restoration behaviour.
6. Replacement verification where feasible.
7. Failure recovery.
8. Applications used for compatibility testing.

The initial compatibility matrix should include at least:

- Windows Notepad;
- a modern browser text field;
- a browser content-editable field;
- Microsoft Word or a comparable rich-text editor;
- IntelliJ IDEA editor;
- Visual Studio Code editor;
- a terminal or console input field;
- an application that does not expose selected text normally.

The utility must prefer an honest unsupported result over unreliable synthetic input.

---

## 21. User-facing invariants

The following statements must remain true from the user's perspective:

1. LexiSwitch does nothing until explicitly invoked.
2. LexiSwitch processes only the text the user selected.
3. LexiSwitch never requires the user to remember layout order.
4. LexiSwitch never requires repeated shortcut presses to find a candidate.
5. LexiSwitch never treats the current layout as proof of the source layout.
6. LexiSwitch shows source and target before modifying text.
7. LexiSwitch shows all distinct candidates in one chooser.
8. LexiSwitch does not modify text while the user browses candidates.
9. `Enter` applies only the selected conversion.
10. `Ctrl+Enter` applies and requests the selected target layout.
11. `Esc` leaves the original text unchanged.
12. Long text can be inspected without placing the entire text in every list row.
13. No ordinary typing history is collected.
14. No selected text or candidate content is written to logs.
15. A shortcut conflict does not silently disable or terminate the application.
16. No-conversion and failure are visibly different conditions.
17. The application can be operated entirely from the keyboard.
18. The release binary does not contain an active general keystroke-monitoring subsystem.

---

## 22. Definition of done for the pivot

The product pivot is complete when:

- last-word correction has been removed;
- continuous keystroke observation has been removed;
- the release build is not blocked because of typing-monitor behaviour in the tested endpoint-protection environment;
- repeated-press cycling has been removed;
- one chooser displays all distinct candidates;
- self-to-self conversions are excluded;
- unchanged candidates are excluded;
- duplicate candidates are deduplicated;
- current Windows layout is no longer treated as the source;
- source and target are visible for every candidate;
- compact and expanded previews work;
- `Enter`, `Ctrl+Enter`, and `Esc` behave as specified;
- selected text is not modified before commit;
- replacement no longer depends on per-character `SendKeys` synthesis;
- shortcut conflicts are recoverable without application termination;
- the tray accurately displays enabled, disabled, conflict, and error states;
- fewer than two installed layouts produce a disabled explanatory state;
- no selected text or candidate text is logged or persisted;
- no network capability is introduced;
- configuration migration is defined and tested;
- automated tests cover conversion invariants;
- end-to-end tests cover the complete user-facing workflow;
- release-build behaviour is explicitly verified.

---

## 23. Implementation choices intentionally left open

The planning agent may propose implementation alternatives for:

- Windows UI framework;
- exact command-palette dimensions;
- exact preview truncation algorithm;
- exact candidate ranking formula;
- selection acquisition API;
- replacement API;
- clipboard fallback mechanics;
- focus restoration mechanics;
- layout-switching API;
- configuration storage format;
- tray icon artwork;
- internal module boundaries.

Those implementation decisions must preserve the product requirements and user-facing invariants defined in this document.

The planning agent must not reintroduce:

- background typing observation;
- last-word reconstruction;
- repeated-press cycling;
- hidden conversion direction;
- automatic unconfirmed text modification;
- dictionary or model dependencies;
- silent shortcut reassignment;
- implicit layout switching.