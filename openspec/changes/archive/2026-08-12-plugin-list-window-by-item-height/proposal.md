# A plugin list's window is resolved in rows, so a row may be more than one line

## Why

`tests/session_list_pane_handover_gap.rs` refuses the session list's handover on one
structural row, and it is the row every other outstanding one is a function of:

> `the-window-is-the-list-widgets` — the native pane hands its nodes to a ratatui
> `List` and reads `ListState::offset()` back for **four** things: which rows are on
> screen, the `▲ N` / `▼ N` indicators on the border, the click hitboxes, and the index
> the pending-spawn placeholder is inserted at. A seated plugin pane windows by the
> kernel's shared rule over flat children, whose count differs because the plugin's
> index counts the headers the native item folds in.

Two different problems hide in that sentence, and only one of them is about scrolling
*policy*.

**The one this change closes is identity.** The native pane's list item is `optional
repo-group header + session row` — **one** item, one hitbox, one index. The plugin
flattens the same content into `header, row, header, row, row, …`, so a session's index
in the tree is not its index in the kernel's `rows` array, and the drift grows with
every group. That is not a policy disagreement; it is the plugin being unable to say
"these two lines are one row" at all.

The tree can already *express* the grouping — a `column` of two lines is a perfectly
good child of a `list`, and it already gets one rect, one hitbox and one index. What it
cannot do is **scroll**: the kernel resolves a selected list's window with
`ui::visible_window`, which counts children and assumes each is one line. Give it items
two lines tall in a ten-line pane and it hands back ten items to draw in ten rows, of
which five are painted and five are clipped away — including, when the cursor is low
enough, the cursor's own. So the grouping is expressible and unusable, and the pane that
needs it is the one pane whose selection decides what the central pane, the info column,
the file viewer and the code review are all showing.

**The one this change does not close is the scrolling rule.** thurbox's shared rule
keeps the cursor near the middle with a small margin; ratatui's `List` holds its offset
until the cursor leaves the viewport. Both keep the cursor visible and they disagree
about which rows sit beside it. Converging them is a decision with three native panes
and every plugin list downstream of it (`docs/PHASE4-PANE-READINESS.md` §13, ADR-39,
ADR-60), and it is the handover's to take — with a measurement — not a side effect of
teaching the window about heights. This change is written so that it **cannot** take it
by accident: the rule is generalised over item heights in a way that provably reduces to
today's rule when every item is one line, which is every list that exists today.

## What Changes

- **`ui::visible_item_window`** — the layer's windowing rule, generalised from "N
  children in H rows" to "N items of declared heights in H rows". It is the same
  arithmetic: the same margin, the same clamp to the list's tail, the same slice —
  measured in rows instead of in items.
- **`ui::visible_window` becomes a thin wrapper over it** (`|_| 1`), so there is one
  implementation rather than two that can drift, and the three native callers that
  window uniform rows are untouched at their call sites.
- **A plugin list's window is resolved through the general rule**, from each child's
  rendered height. A list whose children are one line each is windowed exactly as
  before; a list with a taller child now keeps its cursor on screen instead of clipping
  it.
- **A declared scroll track measures in rows too**: whether the list overflows, how long
  the track's content is, and where the thumb sits are all row quantities. Identical for
  a list of one-line children, correct for one with taller items.
- **Nothing about any existing tree changes.** No bundled plugin, no native pane and no
  recorded oracle is edited: every list that exists today has one-line children, and the
  generalisation is proved to be a no-op for them by a test that walks every
  (total, selected, height) triple in a range and requires the two forms to agree.

## Non-goals

- **Converging the two scrolling rules.** The native session list keeps its ratatui
  `ListState` and its sticky offset; the shared rule keeps its margin. This change makes
  the shared rule *able* to window the session list's item shape, not *equal* to the
  widget's policy. `the-window-is-the-list-widgets` therefore stays **blocked**, with
  its `stands` text rewritten to say which half moved.
- **Changing the session list's tree.** `ui::project_list::session_list_tree` still
  emits a header and a row as two children, and the bundled plugin still flattens them.
  Adopting the item shape moves the recorded goldens, which is the handover's change to
  make and to justify — this one must leave every recording byte-identical, and does.
- **A new node kind.** A multi-line row is a `column` inside a `list`, which the catalog
  already admits and the renderer already lays out. Adding an `item` node would be a
  second spelling of a container that exists.
- **Telling a plugin a height.** The window stays the kernel's, resolved from the rect
  the plugin is never shown, for the reasons ADR-26, ADR-29 and ADR-30 each rejected
  reporting geometry back into a VM.
- **`ViewNode::stacked_row_count`.** The content-derived seat still sizes itself from the
  number of children an occupant stacks, not from the rows they render as. It is
  width-free by construction and the height walk is not; no bundled pane puts a
  multi-line item in a content-sized seat. Recorded as a limitation rather than closed.

## Gate

No compile-time gate. `ui::visible_item_window` and the renderer that calls it are in
`src/ui/`, which is in every build — the `plugins` Cargo feature gates the VM, not the
view-tree renderer (Phase 0's exit criterion; the module note in
`src/ui/plugin_pane.rs`). Both builds are verified.
