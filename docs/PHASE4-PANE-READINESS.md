# Phase 4 pane readiness — what a bundled pane still cannot do

Phase 4 of the v2 migration ports thurbox's panes to plugins, easiest first:
info panel, tasks, automations, file viewer, global search, code review, session
list. The honest question each port has to answer is not "did it render" but
**did the plugin API suffice, or did it have to be widened** — because that is
the only real evidence about whether a third party can build a pane like ours.

This document is that audit, run against the **info panel** (order 1) before
porting it, with every claim traced to the code that makes it true. It is a
worklist, not a design: each row is a gap someone has to close, with the
cheapest honest closure named.

Status of the audit: **four gaps closed** (inline lines, commit `6e0c7cc`;
style tokens and gauges, ADR-26; kernel state, ADR-27) and **one half open**
(§5). The info panel has now been ported twice — first to the view tree in every
build (ADR-26), then reproduced as the **bundled `info-panel` plugin** (ADR-27) —
so the rows below are no longer predictions. Where a prediction was wrong, it
says so.

The plugin is a reproduction, not a replacement: the native pane is still what
thurbox draws, and `tests/bundled_info_panel.rs` asserts the plugin's view tree
*equals* the native pane's, so the two cannot drift while both exist. Two costs
the port did not pay off are recorded in §7 rather than closed.

## 1. Closed: a line of differently-styled runs

`ui.row` splits its area into equal shares
(`Constraint::Ratio(1, n)` in `ui/plugin_pane.rs`) and a single `ui.text`
carries exactly one style. Between them they could not draw
a muted `Name:` label followed by an unmuted `demo` — which is what
`append_session_section` is eight of, and what every list row in thurbox is.

Closed by `ViewNode::Line`: runs packed on one row at their own display width,
holding only nodes whose width follows from their content. See
`openspec/specs/plugin-host/view-tree/spec.md`.

## 2. Closed: no host binding read kernel state

`plugin::capabilities::build_module_table` granted exactly four things: `name`,
`log`, the `state*` trio over the plugin's own key/value namespace, and the `ui`
constructors. **There was no binding through which a plugin could read a session,
a task, an automation, or anything else the kernel owns** — so a pane that
renders kernel data could not be written at all, not badly, not at all. The
session-list spike hit this too and had to *model* a `sessions()` binding to take
its measurements.

**Closed by a published snapshot** (ADR-27), on exactly the shape this row
proposed: `session::pane_context` holds pure data plus a process-wide
`RwLock<Option<PaneContext>>`, `app` publishes it on the tick, and `plugin`
reads it when a plugin calls a reader. No new architecture edge, no plugin call
on the UI thread. Both properties the row said had to be designed were:

- **The read is capability-gated — three of them.** `sessions`, `metrics` and
  `automations`, each gating one reader. The row proposed one (`sessions`);
  porting the pane showed one was wrong, because the pane also draws host CPU and
  scheduled automations, and a plugin that wants a session name must not have to
  demand host telemetry to get it. The capability list is the install prompt.
- **Publishing is not per-tick work.** Two gates: nothing is built unless a
  running plugin holds a state capability, and nothing is written unless the
  value changed. Asserted on the `pane_context_builds` / `pane_context_publishes`
  counters, not in prose. The row proposed an input signature on the
  `session_order_signature` pattern; ADR-27 rejected it with a reason — over
  these inputs a signature must touch every field the snapshot touches, so it
  saves allocations rather than traversal, and it adds a second description of
  the snapshot's dependencies that can drift from the snapshot.

**What the snapshot does and does not carry** turned out to be the whole design.
A VM loads no `os` and no path library, so the kernel resolves what a plugin
*cannot compute*: a countdown (never an absolute instant), a directory's display
name (never a path), a parent session's name (never only its id), and each
status's glyph **and style token**. Everything else is a number, and the plugin
composes every string it draws. Publishing `"8.0/16.0 GB"` would have made the
port trivial and worthless — the plugin would have been arranging strings the
kernel composed, which proves nothing about a third-party pane.

## 3. Closed: five style tokens cannot address the palette the pane uses

`StyleToken` offers `accent`, `muted`, `danger`, `success`, `warning`, mapped in
`ui::plugin_pane::token_color` onto `accent`, `text_muted`, `danger`,
`status_idle`, `status_working`. The info panel draws from a wider set of
**distinct** palette fields: `text_secondary`, `role_name` (the agent name),
`branch_name` (repos), `status_blocked`, `status_done`, `status_unreachable`.

The consequence is specific, not aesthetic: a plugin **cannot draw a session's
status dot in that status's colour.** `Done` and `Unreachable` have no token at
all, and `danger` is a separate palette field from `status_blocked` — equal in
the default theme, not equal by definition, and a custom theme may set either
alone.

**Closed by eleven more tokens** (ADR-26), each named for and resolving 1:1 onto
the `ThemePalette` field it addresses: `secondary`, `role`, `branch`, `added`,
`border`, and one per session status. The audit proposed closing only the status
roles and calling the rest "weaker cases"; porting the pane showed that was the
wrong line to draw. A weaker case still has to be *drawn*, and a pane that
approximated `role_name` with `accent` would not have been byte-identical — so
the criterion, not taste, decided it. The direction of the constraint held: no
node may name a colour.

Two overlaps are deliberate. `warning`/`status_working` and
`success`/`status_idle` resolve alike today, because the first of each pair is a
token a plugin picks for *its own* meaning and the second is the token for *the
kernel's* session status. Collapsing them would leave a pane drawing status
indicators picking `warning` for working and `status_blocked` for blocked.

## 4. Closed: a gauge needs a width the tree does not carry

`info_panel::render_gauge_lines` right-aligns its suffix by computing
`padding = width - label - right` and sizes its bar to `width - 2`. Both need
the **resolved pane width**, and a plugin has no access to one: the view tree
carries no geometry, and no binding reports a pane's rect.

This blocks every gauge in the pane — session CPU, system CPU and RAM, the agent
context window, and each account-usage window — which is the majority of the
panel's visual weight.

There are two ways out and they are not equivalent. A **`gauge` node** (label,
percent, optional suffix) keeps geometry inside the kernel, is trivially
theme-aware, and is the shape ADR-V14's widget set already assumes; it also
bounds what a plugin can ask for. Reporting the **resolved rect back to the
plugin** is the general answer and the worse one: it makes rendering
width-dependent, which means a resize has to re-enter the VM before the frame
that needs it, and a plugin that mis-measures produces a broken pane rather than
a refused node. Prefer the node.

**Closed by the node** (ADR-26). One thing the audit got wrong and the port
found: a gauge is *not* reliably two rows. When `label + suffix` exceeds the
width the padding is zero and v1's header **wrapped**, pushing the bar down — so
the node's height is `header rows + 1`, not `2`. The first implementation clipped
that header, and the differential test against the retained pre-port renderer
caught it. That is the argument for writing the oracle rather than eyeballing the
port.

The pane now needs no width at all: `ui::info_panel::info_tree` takes no area,
which `the_tree_carries_no_geometry` asserts. That, not the gauge itself, is the
property a plugin would need.

## 5. Closed: the layout seats N panes, and so does the keyboard

Unchanged by the info-panel port: the pane is read-only and takes no keys, so it
neither needed nor exercised this.

**Closed.** The layout no longer hosts a single pane. `ui::layout` divides the
screen with a workspace tree (ADR-24), the right column holds one region per
*visible* plugin pane (`PanelAreas::plugin_panes`), and
`App::render_plugin_panes` draws each of them. Two bundled panes can be on
screen at once, which is what `two_plugin_panes_both_reach_the_screen` in
`src/app/acceptance.rs` asserts.

**Closed by the keyboard half** (ADR-28). What was open was not cosmetic:
`App::toggle_plugin_pane` mutated `plugin_panes.first_mut()`, and with `hello`
and `info-panel` both declaring a pane that meant **the pane §2 shipped could not
be put on screen by any key** — only by `thurbox-cli command run
info-panel.info.show` or by editing the stored choice. So the answer to "did the
port work" was, for a keyboard user, no.

`Action::TogglePluginPane` now toggles directly with one declared pane and opens
`Modal::PluginPanes` with several (none: it does nothing). The alternative this
row proposed — generated per-pane commands as *chords* (ADR-V21) — was rejected
with a reason: `session::Action` is a fixed enum whose order indexes the F1
editor's rows, so generating variants per discovered pane would make the
keybinding namespace depend on which plugins are installed. ADR-V21's generated
commands remain the right answer for the *name-addressed* case and already exist
headlessly; the picker is the answer for the keyboard.

**And the related measurement is answered.** `render_all_panes_collected` no
longer renders a pane the kernel is hiding: `session::pane_visibility` publishes
the hidden set on the tick (change-gated, counted by
`pane_visibility_publishes`) and the host consults it before entering a VM. The
cost this removes is exactly the one the motion work refused to pay for a hidden
pane, and it was being paid by every default install with two bundled panes. The
skip is asserted on `PluginHost::render_calls` rather than on the returned
results, because a pane filtered before the call and one rendered and discarded
produce the identical list — which is how the discarding version survived a
review at all.

One cost accepted in exchange: a hidden pane's tree goes stale, so unhiding shows
its last tree (or `loading`) for up to one worker cycle. That is §7's staleness,
now paid once on a keystroke instead of every second forever.

## 6. What the audit implies about ordering

The migration plan calls the info panel the easy one because it is read-only and
has no input. That is true of its *input* surface and misleading about its
*data* surface: it is the pane with the most kernel state and the most geometry
per line in thurbox. Of the five gaps above, the tasks pane needed §2 and §5 and
neither §3 nor §4 — so it would have been the cheaper first port, and §2 is now
closed for it too.

So the cheapest first bundled pane was not the first row of the table. That is a
finding about the plan, not a proposal to change it — but the next person should
weigh gap §4 before assuming "read-only" means "easy".

Two further findings the port produced, for whoever ports the next pane.

**A missing gap: nothing in the catalogue wrapped.** The audit listed geometry
(§4) but not *reflow*. `ViewNode::Line` clips at one row by construction, while
every row of the info panel was drawn by a `Paragraph` with `Wrap { trim: false }`
— and it must be, since `Activity` and `Signal` carry agent-supplied text of
unbounded length. Closed by `ViewNode::Paragraph` (ADR-26). The lesson
generalises: the audit was written by reading what the *catalogue* offers, and
the gap was in what the *pane* relies on. Reading the pane's ratatui calls, not
the node list, is what finds these.

**The pane is now a plugin.** The rendering claim was narrow on purpose — the
catalogue could express this pane's *rendering*, not that a third party could
have written it — and closing §2 is what made the wider claim available. The
bundled `info-panel` plugin produces the identical view tree from three declared
capabilities and nothing else.

## 7. What the plugin port cost, and did not pay off

Two things the bundled pane made measurable rather than arguable. Neither is
closed, and neither should be closed by the next port either without deciding it
deliberately.

**The plugin pane is up to a second behind.** The render worker polls on a
~1 s cycle (`PLUGIN_RENDER_SLICE` × `PLUGIN_RENDER_SLICES` in `src/main.rs`), so
a live gauge in the plugin's copy of the pane lags the native pane's. Nothing
about the snapshot causes this — it is republished on the tick — and nothing about
the pane hides it: side by side, the plugin's CPU bar visibly trails.
`docs/SPIKE-SESSION-LIST.md` already fixed **event-driven render** as a condition
of the session-list port; this is the second pane to want it, which makes it a
scheduling decision rather than a session-list detail.

**Every plugin will reimplement `format_bytes`.** The snapshot publishes raw
numbers on purpose, so the bundled pane carries eight formatters in Luau
(`humanBytes`, `formatBytes`, `formatBytesPair`, `formatCost`, `formatDuration`,
`formatTokens`, `formatCountdownSecs`, `formatDueIn`) — about 80 lines that any
pane showing a byte count or a duration will write again. A `thurbox.format.*`
helper table would fix it. It is deliberately not added here: it should be
designed from two or three panes' needs rather than one, and adding it in the same
change would have destroyed the evidence that a plugin can own its presentation
at all. Whoever ports the second pane is the right person to decide.

A third thing worth recording because it did *not* happen: **the view tree needed
no widening.** An independent consumer needed `list`, `paragraph`, `divider`,
`gauge` and `text` with eight tokens, every one of which ADR-26 had already added
for the native port. That is the confirmation ADR-26 could not give itself.
