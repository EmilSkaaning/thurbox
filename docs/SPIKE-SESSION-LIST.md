# Spike — can the session list be a plugin?

The v2 plan moves every user-visible pane out of the binary and into Luau
plugins. The session list is the pane that decides whether that holds: it is the
densest, the most frequently redrawn, and the one whose ordering, nesting and
status rules the kernel currently owns. If it cannot meet the frame budget as a
plugin, the retreat is a kernel `sessionList` surface that a plugin only
configures — and that retreat has to be discovered before six other panes are
built on the opposite assumption.

This document records the measurement, not a feature. The code that produced it
was throwaway and is not in the tree.

## Verdict

**The session list can be a plugin, subject to three conditions.** The
protocol's per-frame cost is a small fraction of the budget at realistic session
counts, the tree fits the node bound with wide headroom, and an unchanged
re-push provably costs no repaint. But the pane cannot be built until the view
tree can express a styled row, it must not own its own selection cursor, and its
render must be triggered by events rather than by a fixed poll.

| Condition | Why | Where it lands |
|---|---|---|
| The catalog needs a styled-span line node | A `row` splits its area into equal shares, and a `text` node carries one style — so a v1 row (coloured dot, accent marks, bold name, muted activity) is not renderable today | Additive change to `session::view_tree` + `ui::plugin_pane` |
| Selection stays kernel state | If the plugin owns the cursor, every `j`/`k` costs a full render round trip; kernel-owned selection makes the latency bar pass at any session count | Host keeps the cursor; the plugin supplies rows and row identity |
| Render is event-driven | The worker re-renders on a fixed 1 s cycle, so a keypress waits up to a second for its frame and an idle TUI never stops working | Render on "session state changed" and on a consumed key |

## The bar

Four measures, fixed in advance:

| # | Measure | Bar |
|---|---|---|
| 1 | `view.push` rate, 20 sessions at 5 status transitions/s | ≤ 10 Hz sustained |
| 2 | `first_frame_ms` with the default bundled set active | ≤ 115% of v1 |
| 3 | Idle paint rate, no activity | unchanged from v1's ~4 fps floor |
| 4 | Added input→paint latency on selection change | ≤ 5 ms |

## Method

A representative session-list pane was written in Luau against the existing
plugin API (`@thurbox`'s `ui.*` constructors, `render` returning a view tree). It
is a faithful port of the kernel renderer's logic, not a stub: it groups by the
canonical repo-set key, orders groups by their lowest `display_order`, orders
within a group by `display_order` then insertion, nests children parent-first
under `parent_session_id`, and emits a row carrying the status glyph, the tree /
remote / worktree prefix marks, the name, and the agent activity or blocked
notification.

The fixture is 10 / 20 / 50 / 200 sessions across four repo groups, with a mix
of statuses, a seventh of them multi-repo, a quarter on worktrees, a fifth
remote, a third carrying activity text, and every sixth one a child of another.

Six costs were measured separately, median of 200 timed runs after 20 warmups:

- **marshal** — building the Lua table of session rows the plugin reads. This is
  the inbound boundary crossing. No such host binding exists yet, so it was
  modelled with the exact field set the renderer needs (id, name, status, repo
  names, order, parent, remote host, worktree flag, activity, notification).
- **render** — the Luau `render` call, which includes the marshal.
- **convert** — `plugin::view::from_lua`, the outbound crossing.
- **apply** — `PluginPane::apply`, the structural tree comparison that decides
  whether the UI is dirty. This runs on the **UI thread**.
- **paint** — drawing the tree through `ui::plugin_pane::render_tree`, versus
  the native `render_left_panel`, both through the same `Terminal::draw` path.
- **order** — `compute_session_order`, the native comparator, for the baseline.

`marshal` / `render` / `convert` run on the plugin worker thread; only `apply`
and `paint` are on the render loop.

### What the numbers are worth

They were taken on a 4-core machine running at load average 8–9 with swap
nearly full, in the `test` profile — where dependencies, including the Luau VM
and ratatui, build at `opt-level = 0`. A release build on a quiet machine would
be substantially faster. Two independent runs agreed on medians within ~10–30%
but disagreed wildly on p95, so **only medians are reported** and every timing
below should be read as a loose upper bound.

Three of the findings do not depend on timing at all and are exact: node counts,
interrupt-tick counts, and the expressiveness result.

## Results

Times in microseconds, medians of two runs.

| Sessions | marshal | render | convert | worker total | apply (UI) | paint (UI) | native order | native paint |
|---|---|---|---|---|---|---|---|---|
| 10 | 87 | 368 | 109 | 477 | 2.8 | 72 | 6.4 | 113 |
| 20 | 196 | 665 | 209 | 874 | 5.5 | 129 | 10.2 | 182 |
| 50 | 481 | 1653 | 515 | 2168 | 18.5 | 293 | 22.9 | 375 |
| 200 | 4489 | 11048 | 3487 | 14535 | 74.4 | 1086 | 75.2 | 1321 |

Transport floor — a `PluginThread::render` round trip for a one-node tree,
i.e. the channel and thread cost with no compute: **17.7 µs**.

Tree size, exact:

| Sessions | Nodes | Nodes/session | % of `MAX_NODES` (4096) |
|---|---|---|---|
| 10 | 45 | 4.5 | 1.1% |
| 20 | 84 | 4.2 | 2.1% |
| 50 | 202 | 4.0 | 4.9% |
| 200 | 782 | 3.9 | 19.1% |

The node bound is exhausted at roughly **1013 sessions**. Tree depth is 3 at
every size.

Interrupt budget, exact: one render costs **1 190** ticks at 20 sessions and
**13 334** at 200, against a host budget of 200 000 — 15× headroom at the
largest size measured.

## Against the bar

### Bar 1 — push rate. Passes, widely

The protocol pushes one tree per render, so a change-driven trigger gives one
push per transition: 5 transitions/s is 5 Hz, inside the 10 Hz ceiling. The cost
is the part worth stating — five complete render + convert + apply cycles at 20
sessions took **3.9 ms** and **7.7 ms** across the two runs, i.e. **0.4–0.8% of
one second of one core**.

Note that the current worker wiring pushes unconditionally once per second
regardless of change, so today's actual push rate is 1 Hz. Either way the bar
holds.

### Bar 2 — `first_frame_ms`. Not measurable yet

There is no bundled session-list plugin and no Phase 4 wiring; the only bundled
plugin is `hello`. Measuring startup with no session-list plugin active would
satisfy the bar trivially and prove nothing, which is exactly what the bar was
written to prevent.

One structural point is worth carrying forward. The host already starts detached
and the first frame does not wait for it — a pane simply does not exist until
the host arrives. A plugin session list therefore either pops in a moment after
the first frame, which is a visible regression of a different kind, or the first
frame has to block on a VM. That is a design decision Phase 4 owes, not a number
this spike can produce.

### Bar 3 — idle paint rate. Passes on paint, regresses on CPU

100 identical re-pushes of a 20-session tree produced **0 dirty marks**:
`PluginPane::apply` compares structurally and `poll_plugin_renders` marks the UI
dirty only when it reports a change. The demand-driven loop is intact and the
idle paint rate is unchanged.

Idle **CPU** is not unchanged. The worker re-renders every second whether or not
anything moved: 0.87 ms/s at 20 sessions, 14.5 ms/s at 200. v1's idle cost for
the same pane is zero — the order is cached and nothing paints. The magnitude is
small at realistic sizes, but the kind is new, and it is what the event-driven
render trigger above exists to remove.

### Bar 4 — selection latency. Depends on who owns the cursor

If the **plugin** owns the selection, a keypress goes to the plugin, the plugin
updates its own state, and the new tree arrives only on the worker's next render
cycle — up to **1 second** under the current fixed cadence. That misses the bar
by roughly 200×. It is a wiring defect rather than a protocol cost: with a
render triggered by the consumed key, the added latency is the worker's render +
convert plus the UI thread's apply:

| Sessions | Added latency | Bar |
|---|---|---|
| 20 | 0.88 ms | passes, 5.7× margin |
| 50 | 2.2 ms | passes, 2.3× margin |
| 200 | 14.6 ms | **misses**, 2.9× over |

If the **kernel** owns the selection and the plugin supplies only rows and row
identity, a selection change is a kernel-side re-highlight that never enters a
VM, and the added latency is nil at every session count. That is the design this
spike recommends, and it is what turns a conditional pass into an unconditional
one.

## The finding that is not about the budget

The current node catalog cannot express a v1 session row.

A `row` divides its area into equal shares — `Constraint::Ratio(1, n)` — because
a plugin cannot specify widths. A four-cell row in a 40-column pane therefore
renders as:

```text
" ◐        ⇅ ⑂       fix-osc52-Compacting"
```

The status dot occupies ten columns and the session name is truncated. The only
alternative in the catalog is to pre-compose the row into a single `text` node,
which renders correctly:

```text
" ◐ ⇅ ⑂ fix-osc52-tmux  Compacting       "
```

but carries exactly one style for the whole line, collapsing the coloured status
dot, the accent remote and worktree marks, the bold selected name and the muted
activity text into one colour. The status dot is the pane's entire point.

This is an expressiveness gap, not a budget one, and closing it is additive: a
`line` node holding styled spans, or width hints on `row`. It does not change
the measurements — a line plus three or four spans is the same four nodes per
session already measured — but nothing else in Phase 4 can start until it
exists.

Separately: there is currently **no host binding through which a plugin can read
the session list**. The marshalling cost above is real, since it is the actual
`mlua` table-building cost for the exact field set the renderer needs, but the
binding that would deliver it is still to be built.

## Reproducing

The spike was a `#[cfg(all(test, feature = "plugins"))]` module under `src/app/`
holding the Luau pane source, a VM configured exactly as `PluginVm::new`
configures one, the modelled session binding, and the benchmarks. It was
deliberately not committed. Rebuilding it needs only the fixture, the pane
source, and the six timings above; the two exact findings — node counts and
interrupt ticks — are the ones worth re-checking first, since they are
load-independent and settle most of the argument.
