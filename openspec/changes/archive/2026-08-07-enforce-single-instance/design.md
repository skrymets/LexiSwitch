## Context

The program has no instance guard. The consequences are described in proposal.md; the design
question is only how to detect the first instance and what the second should do about it.

Two facts constrain the mechanism. Detection has to be atomic, because two shortcuts clicked in
quick succession is an ordinary thing for a user to do and a check-then-act would let both
instances pass. And it has to be per-session rather than per-machine, because two users signed
in to the same computer are two users, each entitled to their own agent.

## Goals / Non-Goals

**Goals**

- One agent per user session, decided atomically.
- A second launch that teaches the user where the first one is.
- Nothing left behind that could prevent a later start, including after a crash.

**Non-Goals**

- Passing arguments or commands to the running instance. There are none to pass; the product's
  only action needs a selection and a foreground window, which a launcher does not have.
- Cross-session or cross-user exclusion. Explicitly wrong: those are separate users.
- Replacing the running instance. A second launch is far more likely to be an accident than a
  deliberate restart, and killing something that holds a global shortcut on a guess is worse
  than declining.

## Decisions

**A named mutex, not a window search.** `CreateMutexW` with a fixed name reports
`ERROR_ALREADY_EXISTS` on the losing side of a race, decided in the kernel. Searching for the
existing window with `FindWindow` would be a check-then-act: two instances starting together can
both look before either has created its window, and both proceed. The mutex is also released by
the operating system when the process ends however it ends, so a crash cannot leave a lock
behind — which a lock file would.

**Session-local, not global.** The name has no `Global\` prefix, so it lives in the session's
namespace. Two users on one machine get one agent each, which is the intent; a global name would
let the first user signed in deny the second an agent entirely.

**`ERROR_ACCESS_DENIED` counts as "already running".** If the running instance is elevated and
the second is not, the second cannot open the object and gets access-denied rather than
already-exists. Treating that as "free to start" would produce exactly the two-agent situation
this change removes, and treating it as "already running" is wrong only in the rare case where
some unrelated object holds the name. Declining is the safer error.

**The second instance asks the first to show itself.** It finds the running instance's message
window by class name and posts it a request; the running instance answers with a balloon naming
itself and its shortcut. A user who has lost track of the existing agent — which is the common
reason to start it twice — is shown where it is. This is the one place a window search is
appropriate, because it is not deciding anything: the mutex already decided, and a failure to
find the window costs only the balloon.

**Declining is not a failure.** The second instance exits `0`. The user wanted LexiSwitch
running and it is. An exit code of `1` would make a shortcut, a startup entry or a script report
an error for a situation that is entirely fine.

**The conflict state stops being self-inflicted.** With the guard in place, a shortcut conflict
can only come from another application, which is what the state was always describing. Nothing
about it changes; it simply stops being reachable by accident.

## Risks / Trade-offs

**A user who genuinely wants two agents cannot have one** → No use for a second has been
identified, and the costs are concrete. If one emerges, the guard is one call to remove.

**The message-window class name becomes load-bearing for the hand-off** → Only for the balloon.
If the class is renamed and the search silently stops matching, the second instance still
declines to start; the user simply does not get told where the first one is. Worth a test that
the hand-off actually happens, not just that the second instance exits.

**Three test harnesses create a shortcut conflict by starting a second LexiSwitch** → They must
switch to a separate program that holds the combination. This is not a cost but a correction:
they were testing a conflict with itself, and the state exists for a conflict with something
else.

## Open Questions

None. The mechanism is standard and the behaviour on the losing side is the only real choice,
which is made above.
