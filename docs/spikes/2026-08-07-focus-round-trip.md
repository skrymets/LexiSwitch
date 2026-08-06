# Spike findings: the chooser focus round trip

**Date:** 2026-08-07
**Prompted by:** `docs/LexiSwitch-Product-Pivot-and-Replanning-Brief.md` §8, which replaces
immediate replacement with a keyboard-driven candidate chooser

The pivot's interaction model rests on an assumption nobody had measured. This document
records what was *observed*, not what was decided — an observation stays true, a decision can
be superseded.

## Answers

| Question | Answer |
|---|---|
| Can this process take the foreground with its own window? | **Yes**, every run, every target. |
| Can it hand the foreground back to the target? | **Yes**, every run, every target. |
| Does the target's selection survive the round trip? | **Yes** — same range before and after wherever it was readable. |
| Does a paste still replace that selection afterwards? | **Yes**, every run, every target. |
| How much settle does the handback need? | **None measurable.** 0 ms passed as reliably as 300 ms. |
| Is the read path stable per application? | **No.** Chromium exposed no text pattern on one run and a working one on the next. |

**The interaction model holds.** A focused chooser can replace the original selection.

## Why the question gates the model

A chooser driven by `Up`/`Down`/`Space`/`Enter`/`Esc` must receive those keys, and the ways to
do that are closed except one:

| Approach | Verdict |
|---|---|
| A focused top-level window | The only option left |
| `WS_EX_NOACTIVATE` plus a low-level keyboard hook | Prohibited — precisely the hook [ADR-0003](../adr/0003-raw-input-over-low-level-hook.md) and brief §13.5 forbid |
| `RegisterHotKey` for each chooser key | Globally reserving `Up`/`Down`/`Enter`/`Esc`; not a real option |

So the chooser takes focus, and everything downstream depends on the selection outliving that.
A negative result here would not have cost a feature — it would have cost the interaction
model.

## Machine configuration

| | |
|---|---|
| OS | Microsoft Windows 11 Pro, 10.0.26200.0 (25H2) |
| Toolchain | Rust 1.97.1 `stable-x86_64-pc-windows-msvc` |
| Session elevated | **No** |
| Instrument | `lexiswitch-win/examples/probe_focus_roundtrip.rs` |

## Method

The probe creates **its own real top-level window** as a stand-in for the chooser, rather than
letting some other application steal focus. This matters: whether `SetForegroundWindow`
succeeds depends on *who is asking*, and the permitted case is a process giving away a
foreground window it already owns. A probe that borrowed a third-party window would have
measured a different question and produced a falsely reassuring answer.

Per target: open a document, prove it empty with a clipboard sentinel, seed by pasting, select
all, read the selection, take the foreground with the stand-in, pause as a browsing user
would, hand the foreground back, re-read the selection, then paste a marker and read the
document back. The marker test is the one that matters — if the selection survived, the marker
*is* the whole document; if it was lost, the marker landed at a caret instead.

## Results

| Target | Runs | Text pattern | Foreground taken | Handed back | Selection identical | Paste replaced |
|---|---|---|---|---|---|---|
| Notepad (Win32/WinUI) | 4 | yes | yes | yes | yes, 53 = 53 | yes |
| Notepad++ (Scintilla) | 1 | no | yes | yes | n/a | yes |
| Edge (Chromium textarea) | 2 | **unstable** | yes | yes | yes, 53 = 53 | yes |

Notepad was run at settle values of 0, 50 and 300 ms after the handback. All three passed all
four checks. **No settle is required** for the handback on this target — a useful contrast with
the 180 ms `PASTE_SETTLE` that *is* load-bearing before restoring the clipboard.

## Finding — Chromium's text pattern comes and goes

The same Edge window, on the same page, with the same selection:

```
run 1:  selection before focus moves: no text pattern
run 2:  selection before focus moves: readable, 53 chars
```

This is consistent with Chromium activating its accessibility tree lazily once it detects a
client querying it — the first run would have woken it, and the second found it awake. That
mechanism is inference from the outside and was not measured; the *instability* was.

Two consequences worth carrying into the chooser design:

- **The read path is not a stable property of an application.** ADR-0006's detect-by-attempt
  is still correct — it is arguably vindicated — but any compatibility matrix that records
  "browser → clipboard path" is recording one sample, not a fact.
- **The first invocation in a browser may behave differently from later ones.** Anything that
  caches a per-application decision would cache the wrong answer roughly half the time.

The replacement test passed in both runs regardless of which path the read took, which is the
reassuring part: the *outcome* was stable even though the *route* was not.

## What was not tested, and why

- **VS Code — not installed on this machine.** The brief's §20 compatibility matrix names it,
  as does task 8.6 of the retiring change. It cannot be covered here at all. Same class of gap
  as the installed-layout dependency: the machine's software determines what is testable.
- **Windows Terminal — the probe's shape does not fit.** It assumes a document that `Ctrl+N`
  creates, `Ctrl+A` selects entirely, and `Ctrl+C`/`Ctrl+V` round-trip. A terminal shares none
  of those meanings: `Ctrl+N` is unbound, `Ctrl+A` is not select-all, and `Ctrl+C` is
  interrupt unless a selection already exists. Covering a terminal needs a differently shaped
  probe that types at a prompt and selects with `Shift+Home`. Recorded as unmeasured rather
  than assumed either way.

Both gaps are compatibility questions. Neither affects the model-viability answer, which three
structurally different targets — Win32 with a text pattern, Scintilla without one, and
Chromium — agreed on.

## Incidental observations worth carrying

- **A launched editor that is already running hands off to the existing instance** and the new
  process never takes the foreground, so the probe correctly refuses to type. This blocks
  back-to-back repeat runs against Notepad and Notepad++ and is why Notepad++ has one sample
  rather than two. Close the target between runs.
- The clipboard-sentinel emptiness check earns its place again: it is what makes it safe to
  point this probe at a session-restoring editor.
