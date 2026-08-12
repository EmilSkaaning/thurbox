# The session list's window becomes the kernel's own rule, in both panes

## Why

`tests/session_list_pane_handover_gap.rs` refuses the session list's handover on **one**
structural row, and it has been the sole decider since ADR-60 relocated the model:

> `the-window-is-the-list-widgets` — the native pane hands its nodes to a ratatui `List`
> with a `ListState` and reads `ListState::offset()` back for four things: which rows are
> on screen, the `▲ N` / `▼ N` indicators on the border, the click hitboxes, and the index
> the pending-spawn placeholder is inserted at.

ADR-61 closed half of it — **identity**. The kernel's shared rule now windows a list in
rows measured from each child's rendered height, so a pane can express "this repo-group
header and this session are **one** row" as a `column` inside a `list`: one index, one
hitbox, one row that scrolls whole. What it left open is **policy**: the shared rule keeps
the cursor near the middle with a small margin, the widget holds its offset until the
cursor leaves the viewport, and both keep the cursor visible. So at any height where the
list overflows the two panes show a different set of rows *beside* the cursor — in the one
pane whose selection decides what the central pane, the info column, the file viewer and
the code review are all showing.

That is a behavioural difference, not a rendering divergence, and a handover is not
allowed to make one silently (`migration/handover`: "A behavioural difference a handover
exposes is decided, not discovered"). It is the same shape as the **frame** convergence
that specification already requires: a native pane whose frame differs from the host's is
converged onto the host's frame in a change *before* its handover, so the handover can
claim that which code draws the pane changed and nothing else did. This change does for
the window what that rule does for the frame.

It converges in the direction the refusal permits. `migration/phase-4` forbids closing the
row by "redefining the kernel's own windowing rule to match one pane's widget" — that
helper is what every plugin list and three seated panes scroll by. Nothing there says the
**pane** may not adopt the rule, and that is the direction taken: the shared rule is
untouched, the pane changes.

## What Changes

- **The native session list is drawn from its own view tree**, through
  `ui::plugin_pane::render_tree_rows` — the same painter its reproduction goes through —
  instead of being handed to a ratatui `List`. Its window is `ui::visible_item_window`, its
  hitboxes are the painter's row rects, and `App::session_list_state` is deleted with the
  widget.
- **A repo-group header and the session below it become one list item in both trees.**
  `ui::project_list::session_list_tree` emits a `Column` of the two where it emitted two
  flat children, and the bundled plugin emits `ui.column({header, row})` for the same rows.
  The native pane's widget already folded them; now the *tree* says so, which is what makes
  one index mean the same row in both panes.
- **The painter reports the window it drew.** `render_tree_rows` returns the outermost
  list's clipped-row counts alongside its hitboxes, so the `▲ N` / `▼ N` indicators are read
  off the paint rather than off a widget's offset — derived from what was drawn, for the
  reason the hitboxes already are.
- **Clipped-row indicators become shared vocabulary** (`ui::draw_clipped_indicators`),
  painted from those counts. Identical output; a second consumer arrives with the chrome
  row.
- **`the-window-is-the-list-widgets` closes**, and the enumerated divergence it mirrors in
  `tests/bundled_session_list.rs` is replaced by its opposite: the two panes are asserted to
  window a long list **identically**, at the same height, with the same clipped counts.

## The behaviour that changes, stated rather than discovered

The session list scrolls differently. Before: the cursor stays where it is on screen until
it reaches an edge, then the list scrolls by one. After: the list opens `min(height/4, 3)`
rows above the cursor and clamps at the list's tail — the rule the tasks pane, the
automations band, the file viewer and every plugin list already scroll by.

This is a **convergence**, not a preference: after it, thurbox has one windowing rule for
every list it draws, and the session list is the last pane that had its own. The visible
consequence is that moving the cursor into an overflowing list jumps the window once
instead of scrolling row by row from wherever it was left. The cursor is always visible in
both.

## Non-goals

- **Handing the pane over.** `src/ui/project_list.rs` still exists and is still what
  `src/app/view.rs` draws. Three vocabulary rows remain outstanding — the pane's border
  chrome, the pending-spawn placeholder row, and the Unicode-aware trim — and the gate goes
  on refusing the handover on them.
- **Changing the shared rule.** `ui::visible_item_window` is not touched. If this change
  needed the helper to behave differently, it would be the change `migration/phase-4`
  refuses.
- **The placeholder's slot.** `pending_spawn_slot` stays in the pane: it now indexes folded
  items rather than widget items, which is a consequence of this change, but publishing it
  is the placeholder row's change.
- **The empty state.** The native pane still returns early and draws a centred
  `Paragraph`; the plugin still emits two left-aligned rows. `ViewNode::Center` exists
  (ADR-62) and neither has adopted it — that moves a recording and belongs to the handover.
- **A scroll track on the session list.** The native pane never drew one and this change
  adds none; the indicators on the border are what it has always shown.

## Gate

No compile-time gate. Both panes' painter is `src/ui/plugin_pane.rs`, which is in every
build — the `plugins` Cargo feature gates the VM, not the view-tree renderer. The bundled
plugin's half is verified in the default build; the native pane's in both.
