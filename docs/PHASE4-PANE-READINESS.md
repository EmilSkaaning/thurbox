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

Status of the audit: **three gaps closed** (inline lines, commit `6e0c7cc`;
style tokens and gauges, ADR-26), **one open in full** (§2) and **one half open**
(§5). The info panel has now been **ported** — it renders through the view tree
in every build — so the rows below are no longer predictions. Where a prediction
was wrong, it says so. No *plugin*-authored pane beyond `hello` exists yet, and
§2 is why the info panel is not one.

## 1. Closed: a line of differently-styled runs

`ui.row` splits its area into equal shares
(`Constraint::Ratio(1, n)` in `ui/plugin_pane.rs`) and a single `ui.text`
carries exactly one style. Between them they could not draw
a muted `Name:` label followed by an unmuted `demo` — which is what
`append_session_section` is eight of, and what every list row in thurbox is.

Closed by `ViewNode::Line`: runs packed on one row at their own display width,
holding only nodes whose width follows from their content. See
`openspec/specs/plugin-host/view-tree/spec.md`.

## 2. Open: no host binding reads kernel state

`plugin::capabilities::build_module_table` grants exactly four things today:
`name`, `log`, the `state*` trio over the plugin's own key/value namespace, and
the `ui` constructors. **There is no binding through which a plugin can read a
session, a task, an automation, or anything else the kernel owns.**

So a pane that renders kernel data cannot be written at all — not badly, not at
all. The session-list spike hit this too and had to *model* a `sessions()`
binding to take its measurements.

What the info panel would need, minimally, is the active session's
`SessionInfo`: name, status, agent, parent name, remote host, hook-wiring
degradation, OSC activity, last signal, worktree repo and branch, additional
dirs.

**Shape this should take.** The precedent is `session::spawn_contribution`: a
process-wide `RwLock<Option<_>>` in `session`, written by whichever binary owns
the data and read by `plugin` when it builds a binding. Applied here, `app`
publishes a snapshot when the data changes and the binding reads the published
snapshot — no new architecture edge (`plugin` already reaches `session`), no
`plugin` call on the UI thread, and the renderer still cannot reach a VM.
Two properties have to be designed rather than assumed:

- **A read is capability-gated.** `sessions` is a new `Capability`, and reading
  session names, branches and activity text is exactly the kind of reach an
  install prompt has to be able to state.
- **Publishing must not be per-tick work.** The snapshot is rebuilt only when
  its inputs change, on the pattern `App::session_order_signature` already uses
  — otherwise a plugin nobody is looking at costs the idle loop a rebuild every
  tick, which is the regression ADR-V11 exists to prevent.

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

## 5. Half closed: the layout seats N panes; the keyboard still reaches one

Unchanged by the info-panel port: the pane is read-only and takes no keys, so it
neither needed nor exercised this.

**Closed.** The layout no longer hosts a single pane. `ui::layout` divides the
screen with a workspace tree (ADR-24), the right column holds one region per
*visible* plugin pane (`PanelAreas::plugin_panes`), and
`App::render_plugin_panes` draws each of them. Two bundled panes can be on
screen at once, which is what `two_plugin_panes_both_reach_the_screen` in
`src/app/acceptance.rs` asserts.

**Still open.** `App::toggle_plugin_pane` mutates `plugin_panes.first_mut()`, so
`F10` toggles the **first** declared pane whatever the user wanted and a second
bundled pane is unreachable from the keyboard. Phase 4 ships seven panes, so
this has to be decided before the pane migration finishes: whether `F10` remains
one toggle or becomes per-pane visibility with generated
`<plugin>.<pane>.toggle` commands (ADR-V21) — the pane-visibility spec already
describes the latter. Deliberately not folded into the layout change: seating a
pane is geometry, and giving each pane a key is a keybinding decision with its
own surface.

A related measurement worth taking at the same time: `render_all_panes_collected`
renders **every** declared pane each cycle, visible or not. That is correct for
a first pane and wrong at seven, and it is the cost the motion work was careful
to avoid paying for hidden panes.

## 6. What the audit implies about ordering

The migration plan calls the info panel the easy one because it is read-only and
has no input. That is true of its *input* surface and misleading about its
*data* surface: it is the pane with the most kernel state and the most geometry
per line in thurbox. Of the five gaps above, the tasks pane needs §2 and §5 and
neither §3 nor §4.

So the cheapest first bundled pane is not necessarily the first row of the
table. That is a finding about the plan, not a proposal to change it — but the
next person should weigh gap §4 before assuming "read-only" means "easy".

Two further findings the port produced, for whoever ports the next pane.

**A missing gap: nothing in the catalogue wrapped.** The audit listed geometry
(§4) but not *reflow*. `ViewNode::Line` clips at one row by construction, while
every row of the info panel was drawn by a `Paragraph` with `Wrap { trim: false }`
— and it must be, since `Activity` and `Signal` carry agent-supplied text of
unbounded length. Closed by `ViewNode::Paragraph` (ADR-26). The lesson
generalises: the audit was written by reading what the *catalogue* offers, and
the gap was in what the *pane* relies on. Reading the pane's ratatui calls, not
the node list, is what finds these.

**The pane still cannot be a plugin, and §2 is the sole reason.** Every rendering
gap is closed; `info_tree` is geometry-free and could be produced by plugin code
verbatim. What a plugin cannot do is *obtain the `SessionInfo`*. So the honest
claim from this port is narrow — the catalogue can express this pane's rendering
— and the next milestone for the info panel is a capability-gated session
snapshot, not any further widening of the view tree.
