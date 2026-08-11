# Design

## 1. Why the input surface cannot be ported, key by key

The pane's ten `KeyContext::Tasks` actions, each with the host power it needs.
The column that matters is the last one: four of them need a write the
kernel-state channel does not have, and the two that need nothing new cannot
name a row.

| Action | What the native pane does | What a plugin would need |
|---|---|---|
| `TasksNext` / `TasksPrev` | move `task_ui.task_panel_index` | a **view write** — nothing writes a cursor |
| `TasksPreviewDown` / `TasksPreviewUp` | scroll `task_ui.task_preview_scroll`, which is the **central pane's** preview | a view write, plus a surface a right-column pane does not own |
| `TasksNew` | create a task, then focus the central-pane editor | a create binding (`TasksWrite` grants none), a central seat, a text write |
| `TasksOpen` | focus `InputFocus::TaskEditor` | a focus write, plus that seat |
| `TasksRun` | open `Modal::TaskActionPicker`, whose outcomes are *paste a prompt into a running session's PTY* or *spawn a session* | a modal, plus two powers no capability names |
| `TasksOpenRelated` | switch the active session and focus its terminal | a view write |
| `TasksCycleStatus` | `set_task_status` | **nothing new** — `tasks-write` has it |
| `TasksDelete` | soft-delete the task | **nothing new** — `tasks-write` has it |

`FocusTasks` is the pane's own toggle and is ADR-37's second handover
requirement (a manifest binding to an existing action and feature flag), not an
input gap. `Esc` already leaves a plugin pane, kernel-owned, so a pane can never
trap the user.

### The finding: the last two do not survive either

`Capability::TasksWrite` addresses a task **by id**, and the ids arrive on the
published rows, so `Space` and `d` look portable. They are not, because of which
row they would act on.

- A plugin receives a key only while one of its panes holds focus, which is
  `InputFocus::PluginPane`.
- `App::build_tasks_snapshot` computes `cursor_visible` as
  `matches!(self.focus, InputFocus::TaskList) || global_search_preview_kind() ==
  Some(SearchKind::Task)`, and marks `selected` only on that row.

Those two conditions are disjoint. While the plugin can be pressed at, the
kernel publishes no selected row; while the kernel publishes one, the plugin
receives nothing. There is no arrangement of today's bindings under which a
plugin pane's key acts on the row the user is looking at.

This is why the port fails at `j` rather than at the editor, and it is the
sentence the new gate exists to keep true: **a plugin pane's keys and the
kernel's cursor cannot be live at the same time.**

## 2. The editor and the picker: answered, with the evidence

The brief asked whether a plugin could own the central-pane editor and the
trigger-time picker, or whether they stay kernel like the F1 editor does under
ADR-V21. They stay kernel, and the answer does not rest on taste:

- **Seat.** `PaneSlot` is a closed set whose only member is `Right`. The editor
  is drawn into the *central* pane by `App::view`'s task workspace; the picker is
  a centered `Modal`. Neither is a rect a plugin can be given, and the layout
  work to give it one is a decision about `ui::layout`, not about this pane
  (ADR-37 records the same wall for the info panel's own region).
- **Focus.** Both are focus states (`InputFocus::TaskEditor`, and a modal that
  captures input ahead of the global keybinding lookup). Entering one is a view
  write.
- **Text.** `Capability::TasksWrite` states plainly that it grants no creation
  and no editing, because a task's title and description are authored in
  thurbox's own editor. The editor *is* that authoring surface, so handing it to
  a plugin is the write the capability was defined to exclude.
- **Reach.** The picker's two outcomes are typing a prompt into a running
  session's PTY and spawning a session. The capability vocabulary names neither,
  and `Capability::AutomationsWrite` — the widest grant defined — is careful to
  make *running* a request the kernel fulfils rather than an action a plugin
  thread takes. A picker that spawned sessions would be a strictly larger power
  than the widest one that exists.

Recording this is the deliverable, not implementing it: ADR-V21 keeps chrome
that owns the whole interface's input in the kernel, and three of Phase 4's
records now name the same missing pieces (a frame node, a bottom-anchored
region, a central seat). A fourth consumer does not change the answer.

## 3. The cursor: anchor versus appearance

`TasksSnapshot` gains `cursor: Option<usize>`, and the alternatives were:

**Rejected — reuse the per-row `selected` flag as the anchor.** It is
focus-gated, so the plugin's copy would scroll only while the *native* pane held
focus and would jump back to row 0 otherwise. That is a worse divergence than
the one being closed, and it conflates two facts the file-viewer port already
separated: which row the cursor is on (the list's anchor) and whether a row is
drawn as the cursor's (the run's appearance).

**Rejected — publish rows already windowed.** Refused three times before
(ADR-26, ADR-29, ADR-30) and the reason is unchanged: the publisher has no
width or height, the snapshot is built on the tick while a pane's rect exists
only during a frame, and the plugin's pane is a *different rect in the same
layout*, so rows windowed for the native pane would be wrong at the plugin's.

**Rejected — report the resolved rect into the plugin.** The fourth refusal of
the same request. It makes rendering width-dependent, so a resize must re-enter
the VM before the frame that needs it, and a plugin that mis-measures produces a
broken pane rather than a refused node.

**Chosen — publish the anchor, let the kernel window.** `ui.list`'s second
argument already exists and the renderer already resolves the window through
`ui::file_viewer::visible_window`. The plugin gains one line and learns no
dimension.

The bound rule is copied verbatim from the file section: when the cursor falls
past `MAX_TASK_ROWS` the cursor is **not** published, because an index into rows
that were not published would make the kernel's own windowing meaningless.

## 4. Why the native pane is refactored rather than left alone

`tasks_tree` was fed rows the pane had already windowed. If it stayed that way,
the plugin's list (all rows + an anchor) and the native pane's (a window, no
anchor) would be different trees, and the equality test — the port's whole
deliverable — would have to be weakened to "equal up to the window".

So the native pane follows the file viewer: it fits every row, hands the whole
list plus the cursor to the tree, and calls `visible_window` a second time for
the numbers its click hitboxes need. One windowing implementation, in the
renderer, for both panes — two would be two panes disagreeing about which rows
sit beside the cursor.

The cost is that the pane now fits every published row rather than only the
visible ones: at most `MAX_TASK_ROWS` (200) `truncate_ellipsis` calls per paint,
on a pane that is only painted when the UI is dirty. The alternative — fitting
lazily inside the renderer — would move width-dependent work into a node, which
is the model this whole section refuses.

Because the frame must not move, the proof is that no acceptance snapshot
changes and that the pane's own tests (which paint) still pass.

## 5. Why the verdict is a test and not only a document

`tests/global_search_pane_gap.rs`'s reasoning applies verbatim: "the tasks
pane's keys cannot be ported" stops being true the moment someone adds a view
write for an unrelated reason, and nothing would say so. The new gate is one
probe per blocker, each scoped to the declaration it reads rather than to a
whole file, so a change that closes one fails here and is told which.

It is deliberately **not** merged into `tests/teardown_gate.rs`, which answers a
different question — may `src/ui/tasks_panel.rs` be deleted — whose answer is
already no for ADR-37's reason and would stay no either way. One table
answering two questions produces failures that do not say which question moved.

Two probes are worth stating because they are easy to write wrongly:

- The mutual-exclusion probe must read **both** halves — that the snapshot's
  cursor visibility is gated on the tasks focus, and that a plugin pane is a
  *different* focus. A probe that read only the first would report the wall
  closed the moment the gate's spelling changed.
- The "no view write" probe must not merely look for a write-shaped binding:
  `setTaskStatus` is one, and it is not the write these keys need. This is the
  correction ADR-35 already forced on the global-search gate, so the tasks gate
  is written with it from the start.
