## Context

See `proposal.md` — Why, and `specs/case-conversion/spec.md` for what the style must produce.

`render` in `lexiswitch-core/src/casing.rs` has two assembly paths. The prose path keeps the
separators it found. The identifier path (`Style::is_identifier`) discards separator text,
strips non-alphanumerics from *inside* words, joins each line's words with the style's own
delimiter, and keeps line breaks verbatim unless a line option is set.

The abbreviation shares the per-line behaviour but not the rest. It needs every
non-alphanumeric to *split* words rather than vanish from inside them (`openspec/changes` is two
words, not `openspecchanges`), and whether a word is shortened or kept depends on the whole line
— a line in capitals throughout with several words is shortened, anything else keeps its
capitals — which a per-word transform called with one word at a time cannot see.

The case window draws `Style::GROUPS` and sizes and scrolls itself from the resulting line list,
so a fourth group is data, not code.

## Goals / Non-Goals

**Goals:**

- The abbreviation's word rule is its own and cannot leak into any other style's segmentation.
- It honours the digits option and the line options exactly as the identifier styles do, and the
  line breaks it keeps are byte-for-byte the ones written, `\r\n` included.
- Nothing in `lexiswitch-win` changes unless looking at the window shows that it must.

**Non-Goals:**

- No change to `split` or to where any other style finds a word. `HTTPServer` stays one word for
  `snake_case`; the abbreviation keeps its capitals for a different reason.
- No dictionary or length heuristic deciding whether a capitalised word "is" an acronym. The
  rule is structural — how many words the line has and whether it has any lowercase letter.
- No option to keep the delimiters (`F.W.C.`) or to lowercase. One style, one shape.

## Decisions

**A path of its own: `render` hands `Style::Abbreviation` to an `abbreviate` function before
tokenising.** `abbreviate` does:

1. Unless a line option is set, cut the text into lines and the break runs between them, keeping
   each run's characters as written. With `join` or `remove` set the whole text is one line —
   both give the same single abbreviation, since there is no whitespace in the result for a
   joining space to survive as.
2. Per line: drop digits if the digits option is set, then split at every character that is not
   alphanumeric.
3. If the line has more than one word and no lowercase letter, each word gives its first
   character and its later digits. Otherwise each word gives its first character uppercased and
   its later uppercase letters and digits.

The punctuation option has nothing left to do — punctuation never reaches the result — and the
whitespace options likewise, so `toggle_affects` reports all three as having no effect without a
table saying so.

*Alternative considered:* keep the abbreviation on the identifier path, as the first revision of
this change did, and teach `strip_word`/`case_word` the new rules. Rejected: the path's
segmentation is `split`, which is shared by every style, and the shorten-or-keep choice needs the
line, not the word. Both would mean threading abbreviation-only parameters through shared code.
The earlier objection to a separate path — that it would duplicate the identifier per-line loop —
does not hold for this one: that loop's difficulty is reconciling separators with words, and the
abbreviation has no separators to reconcile; it cuts at break characters directly.

`Abbreviation` is therefore *not* in `is_identifier`, and that predicate's doc comment returns to
describing the eight identifier styles only.

**Uppercase with `char::to_uppercase`, all of it.** A first character whose uppercase is more
than one character (`ß` → `SS`) yields all of them. This is what `UPPERCASE` already does, and
truncating to one character would be the only place the program invented a case mapping of its
own.

**"Lowercase letter" is `char::is_lowercase`, "uppercase letter" is `char::is_uppercase`.** A
caseless script (CJK) has neither, so a line of it with several words is shortened to first
characters, and a single word of it gives its first character — the same answer either way.

**Usage identifier `abbreviation`, label `ABBR`.** The label follows the convention that each
label is written in its own style. The identifier is the full word because it is written to disk
and must not change if the label is ever reworded (see `Style::id`).

## Risks / Trade-offs

- [Capitals in mixed text are always kept, so `Hello WORLD` gives `HWORLD`] → this is the rule
  asked for — capitals are treated as already abbreviated — and the result is shown before it is
  applied. The capitals-throughout exception covers the case where shouting is the whole line.
- [A single word in capitals is kept whole, so `UPPERCASE` gives `UPPERCASE`, and `ABBR` is
  marked as having no effect on it] → structurally indistinguishable from `ABBR` or `HTTP`; a
  heuristic to tell them apart would be wrong in both directions. Dimming says so honestly.
- [The window gains a row and a rule and may no longer fit at 150 % scale on a small display]
  → it already scrolls with a visible indicator (manual check D9); re-check D9 rather than
  assume.
- [A stored usage file from this version names `abbreviation`, which an older build does not
  know] → `Style::from_id` already ignores unknown identifiers by design; no migration.
