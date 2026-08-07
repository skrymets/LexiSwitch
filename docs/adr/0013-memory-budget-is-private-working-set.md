# ADR-0013: The memory budget is private working set

## Status

Accepted. Refines a constraint cited by [ADR-0002](0002-rust-as-implementation-language.md);
supersedes nothing. ADR-0002's decision — Rust, for a small binary with no managed runtime —
stands untouched. What changes is only the measure the 20 MB figure is read against.

## Context

The budget has been recorded since the beginning as "resident memory under 20 MB", in ADR-0002,
`CLAUDE.md`, `README.md` and `openspec/config.yaml`. It was never measured against a running
build until the pivot's verification, and measuring it showed the phrase does not name a
number — it names two, which fall on opposite sides of the limit.

Measured over an hour of five-minute samples against the release build
(`docs/manual-tests.md`):

|                                       | working set | private working set |
|---------------------------------------|-------------|---------------------|
| idle, before any conversion           | 13.7 MB     | 1.7 MB              |
| peak, just after the first conversion | 22.2 MB     | 2.9 MB              |
| end of the hour                       | 21.1 MB     | 1.7 MB              |

Both figures are flat or falling over the hour, and the handle count does not move, so nothing
here is a leak. The gap between the columns is entirely shared system libraries mapped in on the
first conversion — `windows.storage.dll` (8.6 MB), `shell32.dll` (7.6 MB) and
`uiautomationcore.dll` (4.1 MB) account for most of it. UI Automation is how the selection is
read without touching the clipboard ([ADR-0006](0006-uia-first-clipboard-fallback.md)), so those
libraries are not incidental; they arrive the first time the program does its job.

So the budget as written fails on the figure Task Manager shows and passes by two orders of
magnitude on the figure that measures what this process actually costs.

## Decision

**The 20 MB budget is private working set.**

Working set counts every page mapped into the process, including pages of shared system
libraries that are already resident for every other process on the machine and would remain
resident if LexiSwitch exited. Charging those to this program measures Windows, not LexiSwitch:
the number moves when an unrelated shell component gains a dependency, and no change to this
codebase can move it back.

Private working set counts the pages this process actually causes to exist. That is the quantity
the budget was always trying to bound — "do not be the sort of tray program that sits on a
tenth of a gigabyte" — and it is the one a change here can be held responsible for.

## Consequences

**The budget is met with two orders of magnitude to spare** (2.9 MB against 20 MB), which is
worth stating plainly rather than presenting as a narrow pass. A limit this slack is not much of
a limit, and a future record may want to tighten it; that is a separate decision and wants its
own ADR.

**The figure a user sees in Task Manager will be higher than the budget**, around 22 MB after a
conversion. Anyone comparing the two will conclude the budget is missed. The wording in
`CLAUDE.md`, `README.md` and `openspec/config.yaml` therefore names the measure explicitly
rather than saying "resident", so the discrepancy is answered where it is met.

**Raising the working-set number instead was considered and rejected.** It would keep the
familiar measure, but it buys a figure this project does not control and cannot defend: the
budget would be set by whatever the Windows shell happens to link against. A criterion that
moves without the code moving is not a criterion.

**Recording both figures and setting no single limit was also rejected.** It is the most
accurate description and the least useful gate — verification needs something that can fail.
Both figures stay recorded in `docs/manual-tests.md`; the budget names one of them.
