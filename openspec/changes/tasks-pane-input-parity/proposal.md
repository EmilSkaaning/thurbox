# The tasks pane scrolls like the native one; its keys cannot be ported

## Why

The tasks pane was to be the first native pane *replaced* rather than
reproduced: bring the bundled `tasks` plugin to full parity — every key the
native pane answers — and then delete `src/ui/tasks_panel.rs`. The v1 behaviour
at stake is the native tasks panel (`src/ui/tasks_panel.rs`, drawn from
`src/app/view.rs`, seated by `RightSlot`, toggled by `Action::FocusTasks` and
gated by `[features] tasks`) together with the ten `KeyContext::Tasks` actions
`src/session/keybindings.rs` declares for it.

Parity is not reachable, and the reason is not the one the attempt expected. The
hard cases were assumed to be the two *separate surfaces* the pane's keys open —
the central-pane editor (`n`, `e`/`Enter`) and the trigger-time action picker
(`r`). Those are indeed unportable, for three walls each. But the port fails
earlier than either, on the **first** key in the list:

- `j`/`k` move `App::task_ui.task_panel_index`, which is view state, and the
  kernel-state channel is read-only by construction. A plugin may now change the
  *records* a pane's keys change (`setTaskStatus`, `deleteTask`) and nothing it
  holds moves a cursor.
- Worse, the two keys that *are* expressible cannot address a row. A plugin pane
  receives keys only while it holds focus (`InputFocus::PluginPane`), and
  `App::build_tasks_snapshot` marks the cursor's row only while the **native**
  pane holds focus (`InputFocus::TaskList`) or a global-search preview is moving
  it. So a focused plugin pane sees no selected row at all: the input path and
  the cursor path are mutually exclusive by construction. A pane whose `Space`
  acted on an unmarked row would be worse than one that takes no keys.

So this change ports what can be ported, closes the one open rendering gap the
pane has left, and makes the input verdict a gate instead of a paragraph —
because a verdict in markdown is a fact about a build that expires without
telling anyone, which is the reason `tests/global_search_pane_gap.rs` exists.

The rendering gap is worth closing on its own. `docs/PHASE4-PANE-READINESS.md`
§8 left the tasks pane two geometry divergences; §9 closed the second one's
*mechanism* for every pane (a list node declaring its cursor, windowed by the
kernel) and the tasks pane never took it up, because the published task section
carries no cursor index. Its per-row `selected` flag cannot serve as one: that
flag is an **appearance** the kernel gates on focus, and a scroll anchor has to
exist whether or not the pane is focused. Splitting the two is exactly the rule
§9 established for the file viewer — the anchor is the list's, the appearance is
the run's — so the tasks pane becomes its second consumer rather than a new
design.

## What Changes

- **The published task section carries a cursor index**, one-based over the rows
  it published, present regardless of which pane holds focus, and absent when
  the list is empty or the cursor falls past the published bound (the rule the
  file section already states). The per-row `selected` flag is unchanged and
  stays focus-gated: one says *which row*, the other says *this row is shown as
  the cursor's*.
- **The bundled `tasks` plugin hands that cursor to `ui.list`**, so the kernel
  windows its copy to the cursor from a height the plugin is never told. §8's
  second divergence is closed for this pane, and its test is replaced by a
  **frame**-equality assertion at a size where the pane scrolls — the standard
  the file-viewer port set.
- **The native pane's tree carries every row plus the cursor**, so both panes
  window through one implementation (`ui::file_viewer::visible_window`) rather
  than two that could disagree. The native pane keeps computing the same window
  as numbers for its click hitboxes, mirroring how the file viewer does it.
- **No key is declared, no pane is replaced, and no renderer is deleted.** The
  plugin stays `capabilities = ["render", "tasks"]` and
  `default_visible = false`; `src/ui/tasks_panel.rs` stays what thurbox draws;
  the `tasks-plugin` teardown row stays blocked.
- **A new gate, `tests/tasks_pane_input_gap.rs`**, records one blocker per key
  the pane's input surface needs and cannot have, and re-derives each from the
  source. It distinguishes a *structural* wall (a plugin pane may not write view
  state) from a *vocabulary* one, so "no rows left" cannot be reached by closing
  only the cheap ones.
- **The audit records the attempt** (`docs/PHASE4-PANE-READINESS.md` §15) and
  **ADR-38** records the anchor/appearance split and the input verdict with
  their rejected alternatives.

## Capabilities

- `plugin-host/kernel-state` — the task section gains its cursor index, under
  the same bound rule the file section states.
- `migration/phase-4` — the phase's geometry rule stops predicting that a
  ported list's copy draws from its first row, and gains the rule that decides
  when a pane's *key* surface may be ported at all.

## Non-goals

- **Replacing the native tasks pane.** Blocked twice over: by the input walls
  above, and independently by ADR-37 — the plugin runtime is an optional
  dependency the release workflow may not enable, so a handed-over pane would be
  missing from every install. Either alone is disqualifying.
- **Declaring `input` or any keybinding on the bundled plugin.** Two of the ten
  actions are expressible as record writes, and neither can name the row it
  would act on while the plugin holds focus. A key that acts on an invisible row
  is a worse pane than one that takes no keys, and `plugin::keymap` already
  refuses to publish a binding that could not be delivered for the same reason.
- **Publishing the cursor's row as *selected* while a plugin pane holds focus.**
  It would mark a cursor no key can move — the appearance would claim a live
  cursor that is frozen — and the rule would apply to every plugin pane reading
  tasks, which is designing an appearance rule from one blocked consumer.
- **Closing §8's other divergence** (a title fitted with an ellipsis). It needs
  a node that clips, which is vocabulary with three recorded consumers and no
  shipping pane that needs it.
- **A view-write channel** (move a cursor, take focus, open a surface). It is
  the wall four of these keys hit and the same one global search hit; it changes
  what a plugin *is*, so it is a design with its own change.

## Impact

- `src/session/pane_context.rs`, `src/app/mod.rs` — the published cursor.
- `src/plugin/kernel_state.rs`, `src/plugin/bundled/thurbox.d.luau`,
  `src/plugin/bundled/tasks/init.luau` — the reader and its consumer.
- `src/ui/tasks_panel.rs` — the tree carries every row and its cursor; the
  window survives as numbers for the hitboxes.
- `tests/bundled_tasks_panel.rs` (divergence 2 becomes frame equality),
  `tests/tasks_pane_input_gap.rs` (new).
- Docs: `docs/PHASE4-PANE-READINESS.md` §15, `docs/ARCHITECTURE.md` (ADR-38).
- No architecture edge, no new capability, no new node kind, and no snapshot
  moves — the native pane's frames are unchanged, which is what the refactor has
  to prove.
