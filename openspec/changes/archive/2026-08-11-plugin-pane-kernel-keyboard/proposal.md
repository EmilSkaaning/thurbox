# A pane may answer one of thurbox's own keyboards, and is focused as that pane

## Why

ADR-50 handed over the first native pane and named what blocks the other five:
**focus**. A seated plugin pane is `InputFocus::PluginPane`, so
`KeyContext::SessionList` / `Automations` / `Tasks` / `FileViewer` never resolve — a
pane may now sit exactly where thurbox's own pane sat (ADR-46), answer the key that
showed it and ride the flag that gated it (ADR-47), and still be a surface the
keyboard cannot reach. It is the one requirement shared by four of the five
remaining handovers, so closing it per pane would be closing it four times.

The two pane-level gates say the same thing from the other side.
`tests/tasks_pane_input_gap.rs`'s `input-and-cursor-are-disjoint` records that the
two keys needing no new host power still cannot name a row, because "a plugin
receives keys only while one of its own panes holds focus, and the published task
row is marked as the cursor's only while the *native* pane holds focus".
`tests/file_viewer_pane_input_gap.rs`'s `no-view-write` records that all seven of
that pane's keys write view state a plugin cannot.

Both rows assume the pane's keys must become the **plugin's** keys. That is the
assumption this change rejects, and rejecting it is what makes the remaining
handovers small: a pane's keyboard is already kernel state operating on kernel
state — `j` moves `App::task_ui.task_panel_index`, `Space` cycles a task's status,
`l` reads a directory, `Enter` launches the editor — and every one of those is
rebindable in the F1 editor and persisted to `keybindings.json`. Handing the
*keyboard* to a plugin would mean re-granting each of those powers as a capability
(a view write, a filesystem read, a process launch), which ADR-38 and ADR-39
refused for reasons that have not changed, and would still leave the surfaces those
keys open — the task editor, the trigger-time picker — kernel-owned.

So a pane declares **which of thurbox's keyboards it is the pane for**. The kernel
keeps the keys, the cursor and the state; the plugin draws. That is the same trade
the seat and the toggle already made: the kernel decides, the plugin declares.

## What Changes

- **`[[panes]]` gains `key_context`.** The kernel key context this pane is the pane
  for, spelled as the kernel spells it (`key_context = "Tasks"`). Validated against
  a **closed set** — the four contexts that scope a *pane's* keyboard
  (`SessionList`, `Automations`, `Tasks`, `FileViewer`). `Global` is refused (it is
  no pane's) and so is `Terminal` (its keys are forwarded to a PTY, not dispatched);
  the error lists what is accepted. Two panes of one manifest may not declare the
  same keyboard, and a `[[keybindings]]` entry naming such a pane is a manifest
  error — a pane cannot give two answers about what one keypress means.
- **Such a pane is focusable without `input`, and is focused as thurbox's own
  pane.** `InputFocus::TaskList` already means "the interface's task list has the
  keyboard"; after this change it does not also mean "and the kernel is painting
  it". So `App::focus_key_context` is untouched, every scoped action resolves as it
  always did, the kernel dispatches it against its own state, and the pane is never
  handed a raw key. Focus *entry* is what moves: the ring stop and the `j`/`k`
  hand-off appear when **either** occupant of that pane's seat is on screen.
- **A pane that holds focus is drawn as focused.** `paint_plugin_pane` painted every
  pane `FocusLevel::Inactive`, so a focused plugin pane had no accent border — a gap
  while every pane was a hidden copy, and wrong the moment a pane is the interface's
  task list. The level comes from one helper the native pane uses too, so the two
  cannot drift.
- **A click on such a pane is the kernel's row selection.** Rows record
  `ClickAction::SelectTask` (and its siblings) rather than `PluginPaneRow`, and the
  pane's whole rect records `FocusPane(<the inherited focus>)`. A click means in the
  plugin's pane exactly what it meant in the kernel's.
- **The published focus fact becomes true for it, and that is not a widening.**
  `plugin-host/input` says a pane is told nothing about its own focus, because a
  published `focused` flag describes the *native* surface a pane reproduces. For a
  pane that declared that surface's keyboard the two are the same pane, so the flag
  is about it — which is what makes the tasks pane's empty-state hint (`no tasks — n
  to add`) correct in the plugin's copy. The rule is unchanged for every pane that
  declares no keyboard.
- **No bundled plugin declares the field.** ADR-47's rule, for its reason: a
  reproduction that inherited the keyboard while the native pane still draws would
  paint two panes as focused and put a cursor in one that a user moves in the other.
  The field ships exercised by tests, ready for the handovers that need it.

## Non-goals

- **No pane is handed over.** No renderer is deleted, no seat is added, no bundled
  manifest changes, and every gate keeps its verdict — `tests/tasks_pane_input_gap.rs`
  and `tests/file_viewer_pane_input_gap.rs` still record their rows as blocked,
  because those rows are about what a *plugin's own* keys could do and this change
  grants a plugin no key at all.
- **No new capability, no new binding, no new node.** A pane that declares a
  keyboard gains no power: the kernel does what it already did, and the plugin
  renders what it already rendered.
- **No view write.** The cursor a key moves stays the kernel's, so `no-cursor-write`
  and `no-view-write` are as true after this change as before. What changes is that
  a pane no longer *needs* one, because the kernel is still the thing moving it.
- **`Terminal` is not a keyboard a pane may claim.** Its keys are forwarded to a
  process, so "the kernel dispatches the action" is false for it, and a pane
  claiming it would silently receive nothing.
- **The two right-column seats are not added here.** A pane's *position* is
  `slot`'s business, and the tasks column and the file-viewer column each need a
  decision this change does not make (the file-viewer column has a second kernel
  occupant — the code review's changed-files list). Each belongs to the handover
  that needs it.

## Impact

- Affected specs: `plugin-host/manifest` (one ADDED requirement),
  `plugin-host/input` (two MODIFIED), `plugin-host/panes` (one ADDED),
  `plugin-host/mouse` (one MODIFIED), `migration/handover` (one ADDED).
- Affected code: `src/session/keybindings.rs`, `src/session/plugin_manifest.rs`,
  `src/plugin/pane.rs`, `src/plugin/lifecycle.rs`, `src/app/mod.rs`,
  `src/app/view.rs`, `src/app/key_handlers.rs`, `src/app/acceptance.rs`,
  `src/plugin/bundled/thurbox.d.luau` (documentation of the focus fact),
  `tests/bundled_manifests.rs`.
- Docs: `docs/ARCHITECTURE.md` (ADR-51), `docs/PHASE4-PANE-READINESS.md` §26,
  `CLAUDE.md`.
- No schema change, no settings change, no new dependency. `keybindings.json` is
  untouched: the actions a declared keyboard resolves are the kernel's own, already
  rebindable and already persisted.
