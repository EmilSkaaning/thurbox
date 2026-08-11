# The left column's two panes are not handed over, and the reasons are executable

## Why

The session list and the automations pane were to be the next two handovers: stop
drawing the native pane, delete its renderer, let the bundled plugin be the pane.
Both attempts fail, and neither fails for a reason a document can be trusted to
keep true.

The session list is the pane ADR-V1 hinges on and `docs/SPIKE-SESSION-LIST.md`
answered *yes, on three conditions*. Two of the three still do not hold, and the
attempt turns up three more requirements the spike could not have seen because
they are about the **handover** rather than about the port:

- **The pane's keys stop resolving.** `App::focus_key_context` maps
  `InputFocus::PluginPane` to `KeyContext::Global`, so all six
  `KeyContext::SessionList` actions — next, previous, open, move down, move up,
  sort A→Z — never resolve while a plugin pane holds focus. The plugin cannot
  substitute for them: `j`/`k` move the **active session**, which is what the
  central pane, the info panel, the file viewer and the code review all show, and
  no capability writes kernel view state. The spike's own answer was "the cursor
  stays kernel state", which is right for a reproduction and is exactly what makes
  the handover impossible: the cursor cannot be kernel state *and* be driven by a
  plugin pane's keys.
- **Reordering and sorting write persisted state no binding names.** `Shift+J`/
  `Shift+K` renumber `sessions.display_order` densely and persist it; `Shift+S`
  sorts within each repo group in one keystroke. The write seam's five operations
  each address a task or an automation; none addresses a session. This is the row
  the brief expected to close by adding a capability, and adding one now would
  ship a third grant with no consumer — the pane still could not name the row to
  act on, which is the shape ADR-38 refused.
- **The module is the kernel's model, more so than the file viewer's.**
  `src/ui/project_list.rs` owns `compute_session_order` (the comparator `App`'s
  `Ctrl+J`/`Ctrl+K` navigate by), `move_in_order`,
  `sort_alphabetically_within_groups`, `resolve_rows` (which builds the snapshot
  the *plugin itself* reads) and `SessionMatch` (global search's session
  matcher). Deleting the renderer deletes navigation, reordering, sorting and
  search.

The automations pane's port shipped five of its seven keys (ADR-41), so its
attempt gets further and stops on its seat rather than on its keys. Its native
pane is the **only** one in the left column, `PaneSlot` names one slot (`right`),
and the left column is the one place in thurbox where a pane's height is derived
from its own content. Worse for a handover: focusing that pane is what turns the
**central** pane into the automation editor plus run history, so a plugin pane
taking focus silently removes the editor, the history and the "open this run's
session" key. And `ui::automations_panel::row_summary` composes the row for the
pane **and** for the `Ctrl+P` list modal, so the module is a model here too.

A degraded session list is a broken product, so both native panes stay. What this
change refuses to leave behind is a verdict in prose: "the session list cannot be
handed over" stops being true the moment someone adds a view write for an
unrelated reason, and nothing would say so.

## What Changes

- **Two gates**, in the shape `tests/global_search_pane_gap.rs` established and
  `tests/tasks_pane_input_gap.rs` and `tests/file_viewer_pane_input_gap.rs`
  follow: one row per thing the handover needs and does not have, each re-derived
  from the source, each tagged by *why* it is missing.
- **A third gap kind, `Wiring`**, for a requirement that needs neither a plugin
  power nor a node: the render trigger and a pane's knowledge of its own focus are
  host wiring, closable without changing what a plugin is. Recording them as
  structural would overstate the wall; recording them as vocabulary would misfile
  them as drawing.
- **The findings that are about the handover rather than the port are pinned
  directly**: that a plugin pane's focus silences a pane's scoped keys, that the
  central pane's mode follows the *native* pane's focus, and that both modules
  are models.
- **ADR-43 and `docs/PHASE4-PANE-READINESS.md` §18** record the two verdicts with
  their evidence, and the ordering of the work that would unblock them.

## Impact

- Affected specs: `migration/phase-4` (two ADDED requirements).
- Affected code: two new gate files under `tests/`; `docs/ARCHITECTURE.md`,
  `docs/PHASE4-PANE-READINESS.md`, `docs/PHASE6-TEARDOWN-READINESS.md`.
- No `src/` change. Both native panes are still what `src/app/view.rs` draws, both
  bundled plugins keep the capabilities they had, and `tests/teardown_gate.rs`
  keeps both rows blocked — now for reasons that fail a test when they stop being
  reasons.
- The gates read the source as text, so they run and mean the same thing with or
  without the `plugins` feature.
