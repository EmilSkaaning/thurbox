# Design

## What this handover has that the info panel's did not

A keyboard. ADR-50 was able to hand over the info panel *because* that pane takes no
input: no scoped actions to resolve, no cursor, no surfaces its keys open. Every one
of the remaining five panes has all three, and the tasks pane has the most of them —
ten scoped actions, a cursor the central pane follows, and two separate surfaces (the
task editor, the trigger-time action picker) reached by its keys.

ADR-51 answered that: the pane declares `key_context = "Tasks"`, is focused as
`InputFocus::TaskList`, and the kernel dispatches those ten actions against its own
state exactly as before. So this change is not about the keyboard; it is about the
four things the keyboard's arrival made visible.

## Decisions

### The seat is named, reversing part of ADR-46

ADR-46 declined to add slots for the tasks panel and the file viewer: *"Both are
right-column occupants and `right` already seats a plugin pane in that column."* That
is true and it is not sufficient. The right column's occupants are drawn in a **fixed
order** — tasks, then file viewer, then plugin panes — so a `right`-slot pane lands to
the right of the file viewer. The native tasks column is to its *left*.

A handover that moved the pane one column over would be exactly the kind of change
this phase's rules exist to prevent, so `PaneSlot::Tasks → RegionId::Tasks` is added,
and the reason is recorded rather than the earlier decision being quietly ignored:
**position within a column is part of the pane**, and only a named seat can express
it. The alternative — letting a manifest declare a position in the column — is the
geometry negotiation `PaneSlot` exists to avoid.

### The hint row stays the kernel's, and that makes seat chrome a concept

The native pane reserved its bottom row, while focused, for `e edit · r run · n new`.
Three ways to keep it, and only one is honest:

1. **The plugin draws it.** It cannot. Those are *rebindable* chords: a user who
   moved `TasksRun` to `x` should see `x`, and no published state carries a keymap. A
   plugin printing the letters it happens to know would print a lie for that user. (The
   native pane hardcodes them today, which is a separate defect and not one to
   propagate into a plugin.)
2. **Drop it.** A real loss of discoverability, in a change whose whole claim is that a
   user does not notice. The same hints exist in the central preview's footer, so the
   loss is small — but "small" is not "none", and ADR-50's precedent is to *decide* a
   difference, not to shrug at one.
3. **The kernel draws it into the seat, above the plugin's tree.** Chosen.

(3) needs one new idea: a seat may carry **kernel chrome**, and the plugin's tree is
laid out in what remains. That is precisely what the native pane did — it subtracted
the footer row from `inner` before rendering its list — so the plugin's content area is
the area the native pane's content had, and the hitboxes come out in the same place.

It is also the mechanism the *next* pane needs: the file viewer's search bar is a
three-row block below its tree whose text is kernel state, recorded by ADR-39 as
needing "the pane-chrome row PHASE4 §13 records". Establishing it here, for one row of
hints, is cheaper than establishing it there for a bordered block with a caret.

The chrome is described as data (`App::pane_hints`), not a closure, so what a seat may
draw stays enumerable rather than becoming "the kernel paints whatever it likes inside
a plugin pane".

### `show_tasks_panel` is deleted, and the focus rescues that flag was doing are kept

ADR-50's rule: the kernel's occupant of a handed-over seat is *deleted*, not switched
off, because `layout_for` carves a seat when **either** occupant wants it — so a flag
nobody paints from carves an empty column. Applied here, `App::show_tasks_panel` goes.

But that flag was load-bearing in a way `show_info_panel` was not: it was the answer
to *"is the pane on screen"* for **focus**. Three places asked it, and each keeps its
question with a new answer:

- the focus ring's tasks stop (ADR-51 already routed this through
  `pane_keyboard_taken`);
- `enforce_feature_visibility`, when `[features] tasks` goes off — the pane hides
  itself now (it declares the flag), but *focus* must still leave it, or the central
  pane keeps drawing a preview for a list that is gone;
- `handle_resize` below 120 columns, where the seat is not carved at all.

The monkey test's invariant is what holds this together, and it becomes stronger
rather than weaker: `focus == TaskList` now implies *a pane provides the task list*,
which in a build with no plugin host is unsatisfiable — so the invariant asserts that
that build can never reach the focus, and a seeded random walk is what looks for a
counter-example.

### `TaskPaneEntry` moves; `TaskRow`, `TaskPaneState`, `task_rows` and `tasks_tree` die

`TaskPaneEntry` is what `App` *builds* — for the pane and, unchanged, for the
published snapshot — so it moves to `src/app/task_state.rs` beside the rest of the
task view state. Same move `SystemMetrics` made in ADR-50, for the same reason: it was
declared in the pane it fed rather than by its owner.

The other four are the *rendering* and go with the renderer. `task_rows` folded the
pane's focus into each row's `selected` flag; the published snapshot already computes
that itself (`build_tasks_snapshot`'s `cursor_visible`), which is why the plugin has
been drawing the same rows all along.

### The oracle keeps the recordings and loses its other two edges

`tests/bundled_tasks_panel.rs` asserted three edges: `native == recording`,
`plugin == native`, `plugin == native` structurally. Two name `tasks_tree`. What
survives is `plugin == recording` — and the twelve `.snap` files must be **byte
identical afterwards**, which is the entire payoff of ADR-42. A `cargo insta accept`
here would convert twelve statements about the pane into twelve statements about the
plugin, so the change reports `git status tests/snapshots/` as empty instead.

### `tests/tasks_pane_input_gap.rs` is retired rather than re-verdicted

Its table asks: *what would a plugin's own keys need to drive this pane?* Five
structural rows, all still true — and after this change, not about anything. The pane
is handed over and its keys were never the plugin's.

Re-verdicting the rows to "closed" would be false (no view write was granted, no
modal surface exists, nothing reaches an agent). Leaving them blocked while the pane
is handed over would read as "this pane is not handed over", contradicting the
teardown gate one directory over. So the file goes, and its content is preserved in
ADR-53's own table — where it is a record of *why the other route was refused*, which
is the durable half. The reasons remain checkable against any future attempt to give a
plugin its own keys, because ADR-51's route did not remove them.

## Rejected alternatives

- **Keep `src/ui/tasks_panel.rs` under `#[cfg(not(feature = "plugins"))]`** so the
  no-plugins build keeps its pane. `migration/phase-4` forbids exactly this, and the
  reason is stronger here than for the info panel: it would leave two task panes that
  differ by build, and the one users install is the one nobody tests hardest.
- **Seed the pane visible** so the no-plugins build's loss is offset by the default
  build gaining a pane. It changes every install's first launch, which a handover may
  not; `show_tasks_panel` initialised to `false` and F5 showed it.
- **Publish the keymap so the plugin can draw its own hint row.** A whole new state
  section for one row of text, and it would put the *kernel's* key names into plugin
  hands as data to re-render — two renderers for one fact, which is the thing the
  hint's own history warns about.
- **Move the hint into the central preview only.** That is option (2) with extra
  words: the hint's job is to be visible in the pane the user is driving.
- **Let the plugin's tree include the hint as a last row.** It would scroll with the
  list, and the plugin does not know the pane's height, so it could not pin it.
- **Give the seat to `right` and reorder the column** so a `right` pane lands where
  the tasks column was. It makes every plugin pane's position depend on which panes
  are installed, and the order is the thing `PaneSlot` fixes so two panes cannot
  disagree.
- **Retire `App::task_pane_entries` and let the plugin read tasks directly from the
  database.** The pane's rows are *derived* view state — the global-search dim/match
  verdicts, the linked-session marker — computed from state a plugin cannot read. That
  is why the section is published at all.
