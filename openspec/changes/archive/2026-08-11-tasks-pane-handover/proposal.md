# The tasks pane becomes the plugin, and the native renderer is deleted

## Why

The tasks pane is the first native pane **with a keyboard** to be handed over, and
everything it needed is now in place:

- its reproduction is **complete** — equal trees at every width, the same painted
  frame when the pane scrolls and when a title is too wide for the column (ADR-52),
  and `tests/tasks_pane_input_gap.rs` records no drawing row outstanding;
- its **keyboard** is answerable by a plugin pane without granting a plugin anything
  (ADR-51): the pane declares `key_context = "Tasks"`, is focused as
  `InputFocus::TaskList`, and every one of its ten scoped actions still fires against
  the kernel's own state, still rebindable in F1;
- its **toggle** and its **flag** are declarable (ADR-47: `FocusTasks`, `tasks`);
- its **oracle** is twelve recordings taken from the native builder (ADR-42/48), so
  the proof outlives the builder this change deletes;
- its **render trigger** is the tasks source moving (ADR-49).

What is missing is the pane's **seat**. ADR-46 declined to add one for this pane on
the ground that `right` already seats a plugin pane in that column — true, and not
sufficient for a handover: the tasks column sits *left of the file viewer*, and a
`right`-slot pane lands to the right of both. A handover that moved the pane one
column over would be a change a user notices.

## What Changes

- **`PaneSlot` grows `tasks`**, mapping to `RegionId::Tasks` — the region the native
  pane occupies. ADR-46's rejection is revisited with its reason recorded: position
  within the right column is part of the pane, and `right` gives the wrong one.
- **`src/ui/tasks_panel.rs` is deleted** (~760 lines, including the pre-port span
  renderer retained as the view-tree port's byte-identity oracle). The tasks column is
  `src/plugin/bundled/tasks/init.luau`, drawn from the `tasks` seat, bound to
  `FocusTasks`, gated by `[features] tasks`, and declaring the `Tasks` keyboard.
- **The kernel's own occupant of the seat is deleted, not switched off.**
  `App::show_tasks_panel` goes with the renderer, so the column is carved by the
  claim alone — ADR-50's rule, for its reason: a flag nobody paints from still carves
  a column, which is the empty-column failure the teardown gate exists to prevent.
- **`TaskPaneEntry` moves to its owner**, `src/app/task_state.rs`. It is the input
  `App` builds for the pane and for the published snapshot, not a rendering type —
  the same move `SystemMetrics` made in ADR-50. `TaskRow`, `TaskPaneState`,
  `task_rows` and `tasks_tree` are **deleted** with the renderer: the plugin builds
  the tree now, and the recordings are what hold it to the pane.
- **The pane's hint row stays the kernel's.** The native pane reserved its bottom
  row, while focused, for `e edit · r run · n new`. A plugin cannot draw it — those
  are *rebindable kernel chords*, which no published state carries — so the kernel
  draws it into the seat, above the plugin's tree, exactly where it was. This is the
  first **seat chrome**, and it is the mechanism a handed-over file viewer's search
  bar would need.
- **`FocusTasks` reports when nothing provides the pane.** With no kernel pane left,
  the action flips the plugin's pane and focuses it; when no pane claims the keyboard
  it says which plugin owns it, rather than doing nothing silently (ADR-50's rule for
  `ToggleInfoPanel`).
- **Global search's task scope reveals the pane through the same door.** Activating a
  task result showed the panel by setting the flag; it now shows the pane, and
  reports when there is none to show.
- **The oracle is rewritten against its recording.** `tests/bundled_tasks_panel.rs`
  loses the `tasks_tree` side ADR-42 predicted it would; the twelve `.snap` files
  become the expectation and are **not regenerated** — byte-identical after the
  deletion is the whole payoff.
- **The teardown gate re-verdicts the row and moves its worked example.**
  `tasks-plugin` becomes the second `ready` pane row; `EXAMPLE_BLOCKED_PANE` moves to
  a pane the interface still draws, which `the_example_pane_is_still_drawn_natively`
  is what enforces.
- **`tests/tasks_pane_input_gap.rs` is retired.** Its question — *can a **plugin's
  own** keys drive this pane?* — is no longer the question the pane poses: the keys
  are the kernel's and always were. Its rows are preserved in ADR-53 rather than
  deleted, and the reasons they recorded still hold for any pane that wants its own
  keys.

## Non-goals

- **No new capability, no new node, no new binding.** The pane draws with what it
  already had and is driven by the kernel. If the handover had needed a widening, the
  reproduction was never equal.
- **The pane is not shown by default.** `default_visible` stays `false`, which is
  what `show_tasks_panel` initialised to: a handover changes which code draws a pane,
  not whether it is on screen.
- **The task editor, the trigger-time action picker and the central preview stay
  kernel.** They are not panes — no seat, no slot, and nothing a manifest could
  claim — and they are reached through the same focus as before. This is the decision
  the brief asked to be recorded: a plugin pane may *open* them, because the kernel
  is what dispatches the key.
- **`--no-default-features` loses the tasks pane**, deliberately, and with it the
  whole task *TUI* surface: the pane is the only door to `InputFocus::TaskList`, so
  the central preview, the editor and the picker are unreachable in that build, and
  global search contributes no jumpable task result. `thurbox-cli task` is unchanged.
  Stage B made `plugins` a default feature precisely so no install is in this
  position, and the teardown gate fails if it ever leaves.
- **The automations pane and the session list are not handed over here.** Each has
  its own remaining rows (a module that is also the kernel's model, drawing gaps),
  recorded in their gates.

## Impact

- Affected specs: `layout/slots` (one MODIFIED), `migration/handover` (one ADDED),
  `migration/phase-4` (one MODIFIED), `migration/teardown` (one MODIFIED),
  `plugin-host/panes` (one ADDED).
- Affected code: `src/ui/tasks_panel.rs` (**deleted**), `src/ui/mod.rs`,
  `src/session/plugin_manifest.rs`, `src/ui/layout.rs`, `src/app/view.rs`,
  `src/app/mod.rs`, `src/app/key_handlers.rs`, `src/app/search.rs`,
  `src/app/task_state.rs`, `src/app/acceptance.rs`, `src/ui/project_list.rs` (a doc
  reference), `src/plugin/bundled/tasks/plugin.toml`,
  `src/plugin/bundled/tasks/init.luau`, `tests/bundled_tasks_panel.rs`,
  `tests/bundled_manifests.rs`, `tests/teardown_gate.rs`,
  `tests/tasks_pane_input_gap.rs` (**deleted**).
- Docs: `docs/ARCHITECTURE.md` (ADR-53), `docs/PHASE4-PANE-READINESS.md` §28,
  `docs/PHASE6-TEARDOWN-READINESS.md`, `CLAUDE.md`.
- No schema change, no new dependency. `settings.toml`'s `[features] tasks` keeps its
  name and meaning; it now gates a pane the manifest binds it to.
