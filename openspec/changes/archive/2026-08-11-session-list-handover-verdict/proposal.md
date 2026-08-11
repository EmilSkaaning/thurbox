# The session list's handover is refused, and the two divergences hiding in its oracle become rows

## Why

With the automations pane handed over (ADR-56) the session list is the left column's
only native pane, and its gate is due a re-verdict: **three of its nine rows stopped
being requirements** on ADR-51's route, which two panes have now taken in practice
rather than in principle.

- `scoped-keys-silenced-by-the-handover` — a pane declaring
  `key_context = "SessionList"` is focused as `InputFocus::SessionList`, so all six
  scoped actions resolve and the kernel performs them.
- `no-active-session-write` — the kernel moves the active session, as it always did.
  **No view write was granted**, which is now the row's job to keep asserting.
- `no-session-record-write` — the kernel renumbers `display_order` and sorts. **No
  session operation was added to the write seam.**

The verdict is still **no**, and this change is where that is re-derived instead of
expiring. What decides it is not in the gate's table at all: it is sitting in the
oracle's doc comments as *enumerated divergences*, which is exactly the shape the
repo's own rule refuses — a verdict written in prose is a fact about a build that
expires without telling anyone, and a test's `///` block is prose.

**The deciding one is the window.** `ui::project_list` hands its nodes to a ratatui
`List` with a `ListState`, and *four* behaviours are derived from that widget's sticky
`offset()`: which rows are on screen, the `▲ N` / `▼ N` clipped-row indicators, the
click hitboxes (a group header travels with the row below it, so a two-line item is one
hitbox), and where the pending-spawn placeholder is inserted. A plugin's list declares a
cursor index and the kernel windows it with `ui::file_viewer::visible_window` over flat
single rows — a different rule, over a different row count, since the plugin's index
counts the group headers the native item folds in. Handing the pane over today would
change **which sessions are visible** whenever the list overflows. In thurbox's primary
navigation that is not a divergence to enumerate.

## What Changes

- **Three rows are re-verdicted closed on a conjunction**, following ADR-54's shape:
  the route is declarable **and** the power it was expected to need is still **not**
  granted. Otherwise "the widening was unnecessary" becomes indistinguishable from "the
  widening happened" in a table that stopped looking.
- **Two rows are added, promoted out of the oracle's enumerated divergences**:
  - `the-window-is-the-list-widgets` (structural) — the four behaviours above, each
    probed. `tests/bundled_session_list.rs` calls closing it "Phase 6 work, not a
    port's"; this is where Phase 6 records what that work is.
  - `non-ascii-whitespace-is-the-kernels-trim` (vocabulary) — the kernel trims an
    agent's activity text with `str::trim`, the plugin with Luau's `%s`, which is not
    Unicode-aware. A no-break space around an activity title survives in the plugin's
    copy.
- **The wrap is recorded as *not* a blocker.** The left column's circular list looked
  like a question this handover would have to answer; ADR-56 settled it. Both ends are
  kernel focuses whoever draws either pane, so the wrap survives this handover
  untouched — and its condition is already "a pane provides that list". A test asserts
  it, so a future reader does not re-litigate it.
- **The verdict test names the new deciders.** It asserted that
  `scoped-keys-silenced-by-the-handover` and `no-active-session-write` were the
  structural rows deciding the verdict; both are closed, so it now asserts
  `the-window-is-the-list-widgets` and `the-module-is-the-kernels-model` — and that the
  closed rows are **not** structural blockers any more, so a regression that reopened
  them fails here.
- **The ordering is asserted, not described.** The window must be settled before the
  chrome, because `▲ N` / `▼ N` are functions of it; and before the module relocation,
  because `resolve_rows` is what feeds both panes.

## Non-goals

- **No handover.** `src/ui/project_list.rs` stays and `src/app/view.rs` keeps drawing
  it. The teardown gate's `session-list-plugin` row stays blocked, and the bundled pane
  stays hidden and keyless.
- **No capability, no node, no publication is added.** Every row that named a grant
  closes *without* one, and this change adds nothing to the host — which is the point
  of re-verdicting rather than building.
- **The oracle's divergence tests are not deleted.** They stay as the *measurement*;
  the gate rows are the *verdict*. Deleting either would leave the other unable to fail
  for its own reason.
- **The module relocation is not done as groundwork.** ADR-54 refused the same thing
  for the file viewer and the reason holds here: a relocation whose destination is
  decided by an unsettled rule (here, what a windowing seam looks like) is motion, and
  `resolve_rows` is one of the things that rule moves.

## Impact

- Affected specs: `migration/phase-4` (two ADDED, one MODIFIED).
- Affected code: `tests/session_list_pane_handover_gap.rs` (three rows re-verdicted, two
  added, two rules rewritten, one added), `tests/bundled_session_list.rs` (the two
  divergence tests point at their gate rows).
- Docs: `docs/ARCHITECTURE.md` (ADR-57), `docs/PHASE4-PANE-READINESS.md` §32,
  `docs/PHASE6-TEARDOWN-READINESS.md`.
- No source change, no schema change, no new dependency.
