# Design

## The question this answers

Four of the five remaining pane handovers are blocked on one sentence in ADR-50: *a
seated pane is `InputFocus::PluginPane`, so `KeyContext::SessionList` / `Automations`
/ `Tasks` do not resolve.* There are only two ways to unblock it, and choosing
between them decides what a v2 pane **is**.

1. **The plugin gets the keys.** A pane declares `input`, receives every keypress
   addressed by binding (ADR-34), and acts. Then it needs a way to *do* what the keys
   did: move a cursor, cycle a status, create a record, read a directory, launch an
   editor, open a modal, prompt an agent.
2. **The pane gets the keyboard.** The pane declares which of thurbox's own scoped
   keyboards it is the pane for. The kernel's actions resolve while it holds focus and
   the kernel performs them against its own state, exactly as before; the plugin draws
   the result.

This change takes (2). The argument is that (1) is not one decision but a queue of
grants, and the queue has already been priced twice: ADR-38 refused a view write for
the tasks pane's `j` (a plugin's cursor and the kernel's would disagree, so `o`/`r`/`e`
would act on a different row than the one highlighted), and ADR-39 refused a
filesystem capability for the file viewer's `l` (the widest grant in the host, and
still insufficient, because the expansion set and the search verdict are the kernel's).
Both refusals stand on their own reasoning. (2) needs none of them: the cursor is still
the kernel's because the kernel is still the thing moving it.

The second argument is the surfaces. Half of the tasks pane's keys *open something
else* — the central-pane editor (`n`, `e`), the trigger-time action picker (`r`), the
related session (`o`) — and the file viewer's `Enter` launches a process. None of those
is a pane, so no pane declaration reaches them; under (1) they would have to be
re-exposed as host operations for a plugin to request. Under (2) they are already
reachable, because the thing dispatching the key is the kernel.

The third is the keybinding editor. Under (2) a pane's keys are the same `Action`s in
the same `KeyContext`, so F1 lists them, a rebind persists to `keybindings.json`, and a
user's existing file keeps working across the handover with no migration. Under (1)
they become `(plugin, pane, id)` keymap entries — a different table, a different
section in the editor, and a user's rebinds silently lost the day the pane changes
hands.

## Why this is not a fig leaf

The objection to (2) is that the kernel keeps the pane's behaviour, so "every pane a
plugin" is only about drawing. Two answers.

The first is where the code lives. A pane's *state machine* is `App` — `task_ui`,
`file_viewer`, `automation_ui` — and its *keyboard* is `session::Action` +
`KeyContext`, neither of which is in `src/ui/`. What `src/ui/<pane>.rs` holds is the
drawing, which is exactly what a plugin takes over. The one exception is
`FileViewerState`, and that it lives in `ui` at all is recorded as a defect by
`tests/file_viewer_pane_input_gap.rs` rather than as the model's home.

The second is the test the phase set itself: *what a bundled plugin can do is what a
third party's plugin can do*. That still holds. A third party may declare
`key_context = "Tasks"` and draw the task list its own way — with a different glyph
set, grouped by status, whatever it likes — and `j`/`Space`/`d` keep working, because
the keyboard is the pane's identity rather than the plugin's implementation. What it
may **not** do is invent a key, and that is the honest boundary: a pane that *is*
thurbox's task list answers thurbox's task keys. A plugin wanting keys of its own
declares `input` and gets ADR-34's addressed bindings, which is the other half of the
model and is unchanged.

## Decisions

### Focus reuses the kernel's `InputFocus`, rather than staying `PluginPane`

`InputFocus::TaskList` already means "the interface's task list has the keyboard". It
did not also mean "and `ui::tasks_panel` is painting it" — that was a coincidence of
there being one implementation. So a seated pane that declared the tasks keyboard is
focused **as** `InputFocus::TaskList`.

The alternative — keep `InputFocus::PluginPane` and teach `focus_key_context` to
return the focused pane's declared context — was rejected after tracing what depends
on the focus. `App::render_task_workspace` (the central preview/editor), the published
`focused` flag, the editor's return path (`self.focus = InputFocus::TaskList` after a
save), `Esc` back to the session list and the focus ring all name the focus by value.
Under that alternative every one of them becomes "the tasks focus, whichever of the
two it is today", an indirection at a dozen sites whose only purpose is to record that
the pane changed hands — and the failure mode is a site that forgets, which reads as a
pane that half works. Reusing the focus makes all of them correct by construction and
leaves `focus_key_context` untouched.

The cost is stated rather than absorbed: `InputFocus` now names a *place in the
interface* and not a renderer. That is the same move ADR-46 made for the seat and
ADR-47 for the toggle, so it is the third consequence of one idea rather than a new
one.

### The declaration is a key context, not "which pane I replace"

`key_context = "Tasks"` names the kernel's own vocabulary for a scoped keyboard, the
way `toggle_action` names an `Action`. A field spelled `replaces = "tasks"` was
rejected for ADR-46's reason: it would name panes rather than mechanisms, and after a
handover "the tasks pane" is the plugin's. A context is not a pane — it is a keyboard —
and several panes could reasonably claim to be the surface a keyboard drives, which is
why the manifest refuses two panes claiming one.

`Global` and `Terminal` are refused. `Global` scopes no pane. `Terminal`'s keys are
translated to bytes and written to a PTY, so "the kernel dispatches the action" is
false for it, and a pane claiming it would sit there receiving nothing with no error to
explain it.

### A pane may not both declare a keyboard and bind its own keys

Refused at manifest validation, per pane: a `[[keybindings]]` entry whose pane declared
a context is an error naming both. The alternative is a delivery order — offer the
plugin the key first, fall through to the kernel — and it produces a pane that can
shadow `d` in the tasks keyboard for every user who installs it, silently, with the F1
editor showing both bindings and no way to tell which wins. A manifest that asks for
both is asking for two answers to one keypress; the host says so at discovery rather
than at the keypress.

The manifest-level `input` capability is **not** refused alongside it: a plugin may
have one ordinary input pane and one pane that inherits a keyboard. Only a binding
*naming the inheriting pane* is the contradiction.

### The focused frame is drawn, and the level comes from one helper

`paint_plugin_pane` painted every pane `FocusLevel::Inactive`. That is a pre-existing
gap — an `input` pane could hold focus with no accent border — and it becomes a
correctness problem the moment the pane is the interface's task list, whose border is
how a user sees where `j` is going.

The level is resolved by `App::pane_focus_level(context)`, which the **native** pane
now calls too. The tasks pane has three levels, not two (`Focused` while the list has
the keyboard, `Active` while the editor it opened does, else `Inactive`), and a second
copy of that rule in the plugin path is how a handed-over pane comes to look almost
right. One helper, two callers, and after the handover one caller.

### A click on such a pane is the kernel's row action

`ClickAction::SelectTask(i)` rather than `PluginPaneRow`, recorded from the row
hitboxes the tree renderer already returns. The alternative — deliver the click to the
plugin as ADR-36 does and let it ask the kernel to select the row — needs the view
write this change exists to avoid. The pane's whole rect records
`FocusPane(<inherited focus>)`, which is what the native pane recorded.

`row.index` is the index into the *tree's* children, and the kernel's row action takes
an index into its own list. They agree because the published section is what the plugin
drew from, and the plugin draws one row per published entry — the same assumption
ADR-36 already makes for a plugin's own rows, and the same one the native pane's
hitboxes make. A plugin that drew a different number of rows would mis-select; that is
a defect in a *reproduction*, and it is exactly what the pane's oracle measures.

### The published `focused` flag becomes true for such a pane

Not a new field and not a new grant: `build_tasks_snapshot` already publishes
`focused: matches!(self.focus, InputFocus::TaskList)`, and after this change that focus
can be the plugin's pane. `plugin-host/input`'s "a pane is told nothing about its own
focus" is narrowed rather than deleted — it is a statement about a *reproduction*,
whose focus is a different thing from the native pane's, and for a pane that is the
surface itself the two coincide. Leaving the flag false would have been the wrong kind
of purity: the tasks pane's empty state reads `no tasks — n to add` only when the key
would work, and in a handed-over pane it does work.

## Rejected alternatives

- **`KeyContext::Pane(String)`** — ADR-34 already rejected the neighbouring idea and
  its reasons hold: `KeyContext` is `Copy` and matched by value, and the keybinding
  namespace must not depend on which plugins are installed. Nothing here adds a
  context; a pane claims one that exists.
- **Infer the keyboard from the seat.** `slot = "left"` would imply the session list's
  keys. Rejected: a third-party pane may legitimately sit in a seat without being that
  pane, and inheriting `d` (delete session) by virtue of geometry is the worst possible
  default. The two declarations stay independent, as `toggle_action` and `slot` already
  are.
- **Grant the view write after all** (`setPaneCursor`, `focusPane`). It is the widest
  power in the host — a plugin could move the user's focus and cursor from a `render`
  it was not driving — and it is not sufficient: the editor, the picker and the process
  launch are still not a pane's to open. ADR-38's rejection of it stands.
- **Deliver the kernel's *action* to the plugin** (`onAction(paneId, "TasksDelete")`)
  and let it perform it through capabilities. This is (1) wearing (2)'s clothes: every
  action still needs a grant to be performable, and now there are two ways to spell a
  key.
- **Let the pane declare the keyboard and the kernel skip its own dispatch**, so the
  plugin can override individual keys. Then a key's meaning depends on a plugin's
  return value, which is unpredictable per install and undiscoverable in F1.
- **Do nothing until a pane is handed over, then wire the focus inside that change.**
  It is the shape ADR-46 rejected for the seat: the first handover would also be the
  first test of the mechanism. Here it would be worse, because the mechanism is
  reusable across four panes and would land as four incompatible bits of focus
  plumbing.
