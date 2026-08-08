## Context

See `proposal.md` — Why, and [ADR-0014](../../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)
for why case is a second front door rather than more candidates in the chooser.

What shapes the approach is what already exists and must not move:

- `Transaction` (`lexiswitch-win/src/transaction.rs`) reads a selection, holds it, and either
  commits a replacement or cancels. It knows nothing about layouts, so it serves this change
  unchanged. Layout-specific behaviour is confined to `Transaction::render`.
- `winui` already holds the window primitives the chooser and the shortcut recorder share.
- The worker thread exists precisely so that a blocking modal window does not freeze the tray.
  The case window is the third window and follows the same rule.
- `lexiswitch-core` is `#![forbid(unsafe_code)]` and its tests must pass on any host. Both new
  modules are pure and belong there.

Three existing pieces are load-bearing in ways this change exposes, and they are covered in
Decisions rather than treated as incidental cleanup: all-or-nothing hotkey registration, the
positional preview window, and the single-shortcut agent state.

## Goals / Non-Goals

**Goals:**

- A case transaction that reuses the read/commit/cancel path verbatim, so the application
  matrix results already gathered continue to apply to it.
- Transform semantics that are exactly specified and exhaustively testable without Windows.
- One rule — dimming — covering both "this style would change nothing" and "this option would
  change nothing", so the window has no second concept to learn or to get wrong.
- Shared window behaviour lifted into `winui` rather than copied, so the fourth window
  inherits it.

**Non-Goals:**

- No change to `chooser.rs`, `candidates.rs`, `convert.rs`, or the `candidate-chooser`
  specification. Freezing them is the point.
- No chaining of a layout conversion into a case conversion. See Open Questions.
- No configurable style list, no user-defined transforms, no per-application behaviour.
- No mouse interaction beyond what a window gets for free.

## Decisions

### The transform is a function of a word sequence, not of a character sequence

Segment into words and separators, then: strip characters from words → resolve line-break
separators → resolve horizontal-whitespace separators → apply the style → join.

The alternative is a chain of character-level rewrites. Rejected because the order of two
character-level rewrites is ambiguous exactly where it matters: removing spaces before
applying `Words Capitalized` destroys the boundaries that style needs, and removing them
after requires the style to have preserved information it has already discarded. Making the
word boundary a first-class thing that survives the deletion of the character that produced
it removes the ambiguity instead of resolving it case by case.

The two separator steps are ordered — lines first — so that `join` produces a space which the
horizontal policy then governs. This composes the way a user expects (`join` + `normalize`
gives one space, not three) and it is why the pairs are two three-valued settings rather than
four independent booleans: `Spaces: Keep | Normalize | Remove` and `Lines: Keep | Join |
Remove`. Exclusivity is then structural and cannot be forgotten by a later edit.

Boundaries are found at whitespace, `_`, `-`, `.`, and a lowercase→uppercase transition, but
**not** between a letter and a digit — otherwise `snake_case("getUserV2")` yields
`get_user_v_2`, which no one wants.

The eight identifier styles drop every non-alphanumeric character unconditionally. An
identifier cannot contain a period, so there is no meaningful alternative; the destructiveness
for prose is acceptable because the user reads the result before committing, which is what the
transaction is for.

### The list is fixed and never filtered; dimming carries the information

`candidates::generate` drops candidates that change nothing, and that is right there: the
candidate set is discovered, its size varies with the text, and an unchanged candidate is
indistinguishable from a wrong guess about the source layout.

Here the opposite holds. The set is fixed at fourteen, every entry has a name the user aims
at, and the promoted-entries feature exists specifically to build muscle memory. A list that
changed shape whenever a toggle was pressed would defeat it. So `generate` returns every row
with a `changes: bool`, and the window renders rather than filters.

The same predicate — "would this change the result?" — answers the toggle question. A toggle
is dim when flipping it produces the same string for the currently selected style. That is
computed by rendering both ways and comparing, which needs no per-style rules table, stays
correct if a style is ever added, and makes the redundant member of an exclusive pair dim
automatically (with `spaces: remove` set, `join` and `remove` lines agree).

Committing a dim row cancels instead of replacing text with itself. A no-op commit would still
put the selection through a clipboard round-trip in the target application, which is exactly
the operation most likely to go wrong for no gain.

### The usage memory is worker-owned and lives in its own file

[ADR-0015](../../../docs/adr/0015-usage-memory-outside-configuration.md) records why it is not
in the configuration file.

Ownership: the worker thread loads it once, mutates it in memory, and writes it after a commit
or a reset. Nothing is shared with the message-loop thread, so no lock exists to be got wrong,
and the file has exactly one writer. The message-loop thread supplies the path in the job and
otherwise never touches it.

Split as `Config` is split: `lexiswitch-core` parses and renders the text and is fully
testable; the binary does the file IO. A missing or malformed file parses as "no memory yet"
rather than as an error — there is nothing in it worth failing over.

Ordering is by count, recomputed only when the window opens. Recency was considered and
rejected on interaction grounds: a recency list reorders after every use, moving the row the
user is reaching for.

### Hotkey registration becomes per-binding

`Hotkeys::register` is documented as all-or-nothing — *"if any binding is unavailable,
everything already registered is released"* — which is the right behaviour for one binding
and silently wrong for two. A conflict on the case shortcut would release the layout shortcut,
disabling the product's primary function to protect a secondary one.

Registration therefore reports per binding, and `AgentState`/`Shortcut` carry both so the
conflict state can name which one is unavailable while saying the other works. The precedence
model in `AgentState::resolve` is unchanged; only the shortcut term becomes plural.

### `changed_span` becomes insertion-tolerant

`preview::compact` positions its window over the region that differs from the original, and
`changed_span` finds that region by comparing position for position. Its own doc comment
states the limit: *"it would be wrong for text where characters were inserted or removed,
which conversion never does."* Case transforms do — `snake_case` inserts `_`, `ß`→`SS` grows
the string — so a positional span would point at the wrong region and the preview would hide
the change it exists to show.

Replacing it with a common-prefix / common-suffix span handles insertion and deletion and
remains exactly correct for the positional case, so the layout chooser inherits the fix rather
than needing a second code path. `Candidate.unchanged` is positional for the same reason and
is simply not a concept the case window has.

### The armed-foreground handshake moves to `winui`

`chooser.rs` documents a burst of activate/deactivate/activate when the foreground is taken
from another process, and arms its "clicking away cancels" rule only after the window has
settled in front. The shortcut recorder reproduced that bug because the rule lived in the
chooser's state rather than in the shared module. This is the third window; the rule moves
before it is written a third time.

The drawing helpers (`draw`, `draw_right`, `rule`) move with it. The list rendering does not —
the two windows draw different things.

### The window is sized to its content, with a scroll fallback

Twenty-six rows is roughly 730 px at 100% scale, which fits; `geometry()` already clamps the
height to the work area. At 150% on a 1080p display the clamp bites, so the list needs to
scroll with the selected row kept visible. This is the uncommon path but not an optional one —
a clamped window that silently cuts off `Words Capitalized` is worse than one that scrolls.

## Risks / Trade-offs

**`Win+Ctrl+Pause` may not register.** `MOD_WIN` parses today, but Windows reserves Win-key
combinations and `Win+Pause` itself opens System properties. → Prove it registers before the
default ships. If it does not, pick another default; the conflict state and recorder handle
the user-facing case but a broken default is a defect.

**Two windows with near-identical chrome will drift.** → Lift the shared behaviour into
`winui` in the same change, not afterwards. The recorder is the precedent for what happens
otherwise.

**Identifier styles are destructive on prose and always offered.** → Accepted deliberately:
the result is visible before commit, and hiding them would need a heuristic about whether a
selection "is" an identifier, which would be wrong sometimes and unexplainable when it was.

**A second file on disk weakens "delete the config to reset everything".** → Documented in
`config.example.toml` and in the ADR; the in-window reset means the common case never requires
finding the file.

**Computing the dim state renders every style twice per toggle.** → Selections are short and
this runs on the worker at human speed. If it ever matters, the second render is cacheable per
`(style, strips)`; it is not worth doing pre-emptively.

**The state file gains a writer while an editor has it open.** → Unlike the configuration
file, nothing else is expected to edit it, and a lost write costs one count.

## Open Questions

- **Chaining a layout conversion into a case conversion.** Deferred rather than skipped: after
  a commit the text is no longer selected and the caret sits at the end, so a second
  transaction cannot reuse the first's selection. Answering it needs the post-commit selection
  state established by measurement. It changes neither these specs nor this task breakdown.
- **Whether the promoted count should be configurable.** Three is chosen for the fixed height
  it gives the window. Making it a setting is additive and can wait for evidence that anyone
  wants a different number.
