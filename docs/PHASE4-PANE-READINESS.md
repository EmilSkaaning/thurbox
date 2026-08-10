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

Status of the audit: **one gap closed** (inline lines, commit `6e0c7cc`), **four
open**. No bundled pane beyond `hello` exists yet.

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

## 3. Open: five style tokens cannot address the palette the pane uses

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

**Cheapest honest closure**: tokens for the status roles, since a status dot is
the one case where the token *is* the meaning and an approximation is wrong
rather than merely different. `role_name`/`branch_name`/`text_secondary` are
weaker cases — they are typographic conventions of one pane, and a plugin
choosing `accent` for an agent name is a defensible reading, not a bug. Note
the direction of the constraint: tokens exist so a plugin follows a theme
switch, so the answer is never "let a plugin name a colour".

## 4. Open: a gauge needs a width the tree does not carry

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

## 5. Open: the layout hosts one plugin pane, and only the first is reachable

`ui::layout`'s `RightSlot::Plugin` is a **single** slot;
`App::render_plugin_pane` draws `plugin_panes.iter().find(|p| p.visible)`, and
`App::toggle_plugin_pane` mutates `plugin_panes.first_mut()`. So with two
bundled plugins installed:

- only one pane can be on screen at a time, and
- `F10` toggles the **first** pane whatever the user wanted, leaving a second
  bundled pane unreachable from the keyboard.

Phase 4 ships seven panes. It therefore cannot be finished on this kernel, and
this is not a pane-porting problem — it is the workspace tree
(ADR-V23), which Phase 0 scheduled precisely so that panes would not have to be
migrated twice. Whatever closes it should also decide whether `F10` remains one
toggle or becomes per-pane visibility with generated
`<plugin>.<pane>.toggle` commands (ADR-V21) — the pane-visibility spec already
describes the latter.

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
