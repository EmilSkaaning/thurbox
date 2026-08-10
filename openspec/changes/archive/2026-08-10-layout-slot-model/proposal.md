## Why

`compute_layout` takes **9 positional boolean/count arguments** and returns a
`PanelAreas` of **10 fixed rects**, with 36 call sites in `ui/layout.rs` alone.
Every pane thurbox has ever added widened that signature, and the v2 proposal
names this as a headline symptom of why nobody outside the project can add one.

It is now blocking. `plugin-view-tree-and-panes` built the whole plugin
rendering contract — view tree, conversion, render round-trip, pane
presentation, renderer — and could not mount a pane, because doing so meant
adding a 10th positional argument and an 11th rect to the very structure v2
exists to dissolve. Bolting it on would have made the slot model harder to
reach, not easier.

## What Changes

- **`compute_layout` takes a params struct** instead of positional arguments,
  so adding a pane stops being a signature change at 36 call sites.
- **Panel areas become a keyed collection** rather than a fixed field per
  panel: the right-hand column holds an ordered list of occupants, and a new
  occupant is a value rather than a struct field.
- **Native panels keep their exact current geometry.** Every existing
  width threshold, ordering, and share is preserved — this is a
  representation change, not a redesign of the layout.
- **A plugin pane can then occupy a slot** without touching the layout
  signature, which is what unblocks the previous change.

## Capabilities

### New Capabilities

- `layout/slots`: how the layout describes panel occupancy — the slot model,
  ordering within a column, and the rules that decide what is shown at a given
  terminal size.

### Modified Capabilities

None yet — v1 layout behavior is not currently described by a spec in this
repo, and this change deliberately preserves it rather than altering it.

## Non-goals

- **No visual change.** Identical geometry at every terminal size; the
  acceptance snapshots must not move.
- **No new panel.** The plugin pane lands in the change this unblocks.
- **No layout features.** No resizing, no user-defined arrangement, no
  persistence of pane sizes.

## Impact

`src/ui/layout.rs` (the signature, `PanelAreas`, and 36 test call sites),
`App::layout_for`, and every `PanelAreas` field consumer in `src/app/view.rs`.
The pinned acceptance snapshots are the safety net: if geometry moves, they
fail.
