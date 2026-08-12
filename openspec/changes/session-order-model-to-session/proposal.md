# The session list's ordering model leaves the renderer that draws it

## Why

`tests/session_list_pane_handover_gap.rs` refuses this pane's handover on six rows. Two
of them are structural and decide the verdict; this change closes one of them, and it is
the one that has nothing to do with drawing:

> `the-module-is-the-kernels-model` — `src/ui/project_list.rs` owns the comparator
> `Ctrl+J`/`Ctrl+K` navigate by, the reorder, the sort, the snapshot the *plugin* reads,
> and global search's session matcher. Deleting it deletes navigation, reordering,
> sorting and search.

That is a v1 layering defect independent of any handover. thurbox's session ordering is
not a rendering decision: `compute_session_order` is documented as the single comparator
"shared by the rendering widget and keyboard navigation **so the two never drift**", and
`move_in_order` / `sort_alphabetically_within_groups` are the primitives behind
`Shift+J` / `Shift+K` / `Shift+S`, which renumber `sessions.display_order` densely and
persist it. None of that is drawing. It lives in `src/ui/project_list.rs` because that is
where the pane that first needed it was written, and the result is that `App`'s navigation
calls **up** into the rendering layer for its own model — four `crate::ui::project_list::`
call sites in `src/app/mod.rs` that a reader of `App` has to leave the kernel to follow.

`migration/handover` already requires a handover to relocate the model its deleted module
also held, "in the same change". That rule assumes the handover happens. This pane's
handover is refused on a separate row — `the-window-is-the-list-widgets`, which this
change does **not** close and does not claim to — so under the rule as written the model
stays in the rendering layer for as long as the window row stands, which is indefinite.
The relocation is therefore hoisted out of the handover and done on its own, exactly as
`migration/handover` already hoists a pane's **keyboard** out of its handover for the same
reason: a change that both relocates a model and moves who draws a pane cannot be
reviewed, because a behavioural difference reads equally as either.

## What Changes

- **A new pure-data module, `src/session/session_list.rs`**, holding the kernel's model of
  the session list: the grouping keys and labels, `SessionOrder` /
  `compute_session_order` and its nesting, `move_in_order` and its block arithmetic,
  `sort_alphabetically_within_groups`, `OrderedSessions`, `SessionMatch`, the resolved
  `SessionRow`, `RowInputs` / `resolve_rows`, and `agent_status_text`.
- **`src/ui/project_list.rs` keeps exactly the drawing**: the view-tree builders, the
  style tables, the ratatui widget assembly, the width fit (`resolve_items`,
  `fit_status_text`, `row_used_columns`), the scroll indicators, and the pre-port span
  oracle. It re-exports nothing — every caller names the model where the model now lives.
- **`App` stops calling into `ui` for its own model.** The four
  `crate::ui::project_list::` call sites in `src/app/mod.rs` and the four in
  `src/app/view.rs` become `crate::session::session_list::`.
- **Behaviour does not change.** This is a relocation: no function's body is edited, no
  signature is widened, and no caller's argument moves. The one deliberate exception is
  spelling — `super::truncate_ellipsis` stays behind with the fit that calls it, because
  the fit is not moving.
- **The gate is re-verdicted.** `the-module-is-the-kernels-model` becomes closed, with its
  probe re-derived from the new location and its `stands` rewritten to say what is now
  true. The two probes that read the moved code by path — `no-pending-spawn-row`'s and
  `non-ascii-whitespace-is-the-kernels-trim`'s — follow the code they measure. Both rows
  stay **blocked**: nothing about either was closed here.

## What Does Not Change

Named, because the value of this change depends on the boundary being argued rather than
drawn where it was convenient:

- **`pending_spawn_slot` does not move.** `migration/phase-4` orders the relocation of
  anything the widget's window feeds *after* the windowing decision, "since what a
  windowing seam looks like decides where those functions live". The placeholder's index
  is exactly such a function: it is an index into the rendered rows that the window then
  offsets, and it is one of the four behaviours `the-window-is-the-list-widgets`
  enumerates. Moving it now would fix its home against a seam that does not exist yet.
- **The width fit does not move.** `resolve_items`, `fit_status_text` and
  `row_used_columns` decide how much of an agent's text fits in a resolved column. That is
  geometry, which `migration/phase-4` keeps in the kernel's *pane*, not in its pure-data
  layer — and the layer that holds it must be one that may know a width.
- **No pane is handed over.** `src/ui/project_list.rs` still exists, `src/app/view.rs`
  still calls `project_list::render_left_panel`, the bundled plugin stays hidden and
  declares no input, and the teardown gate's `session-list-plugin` row stays blocked.
- **The window row is untouched and is now the sole structural blocker.** Measured, not
  assumed: at 40 sessions in four groups and a 30-row pane, the native widget holds its
  offset at 0 until the cursor reaches row 28, while the kernel's shared rule begins
  scrolling at row 3 and is pinned to the list's tail from row 20 onward.
