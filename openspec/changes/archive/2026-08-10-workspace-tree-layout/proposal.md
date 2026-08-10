## Why

`compute_layout` in `src/ui/layout.rs` answers "where does a pane go" with a
**fixed set of named rects**. `PanelAreas` has one field per panel, and the
right-hand column — even after `layout/slots` turned it into an ordered
occupant list — carries exactly **one** `RightSlot::Plugin`, so
`PanelAreas::plugin_pane` is an `Option<Rect>` and `src/app/view.rs` draws
`plugin_panes.iter().find(|p| p.visible)`: the **first visible pane only**.

That is the wall Phase 4 of the v2 migration hit. A plugin may publish several
panes (`set_plugin_panes` takes a `Vec`, `plugin.toml` may declare several
`[[panes]]`), the pane visibility, focus, motion and reload machinery is all
per-pane, and yet the layout can seat one. Migrating a native panel to a plugin
therefore means evicting whatever pane was already there. Adding a second slot
field would repeat the mistake `layout/slots` was written to stop.

`layout/slots` was explicit that it was a *representation* change and would
"preserve existing geometry exactly". It did. This change replaces the
representation itself: the **workspace tree** of `docs/v2/` ADR-V23 (loose
reference on the `thurbox-v2` branch), where space is divided by a recursive
tree of single-axis splits and the five v1 slots become a **synthesized preset
over it**. `docs/v2/` MIGRATION Phase 0 places this before the pane migration
specifically so panes are not migrated twice.

## What Changes

- **A workspace tree describes pane geometry.** A branch is a horizontal or
  vertical split carrying its children in order; a leaf names one region. Every
  rect the frame paints comes from solving that tree.
- **`compute_layout` becomes preset synthesis + solve + projection.** The v1
  vertical bands, the 2-panel/3-panel width thresholds, the left-column
  sessions/automations split and the right-column occupant list all become
  *shapes of one structure* rather than four hand-rolled `Layout::split` calls.
  `PanelAreas` survives as the projection, so its 30-odd consumers are
  untouched.
- **The default preset reproduces v1 exactly.** Each branch hands ratatui the
  byte-identical constraint list v1 passed, so geometry is unchanged by
  construction rather than by inspection.
- **N plugin panes become expressible.** The right column's occupant list holds
  one leaf per *visible* plugin pane instead of a single boolean, and
  `PanelAreas` reports `plugin_panes: Vec<Rect>` so the view can draw all of
  them.
- **Plugin columns past the first cannot squeeze the center away.** They are
  placed only while the center region keeps a minimum width; a column that does
  not fit is hidden, not squeezed, and returns when the terminal widens.

## Capabilities

### New Capabilities

- `layout/workspace-tree`: pane geometry as a tree of splits — the node model,
  how a branch distributes its extent, the synthesized default preset, and how
  many plugin panes the right column can seat.

### Modified Capabilities

- `layout/slots`: the "ordered list of occupants" requirement is restated in
  terms of the tree (a column is a branch; its occupants are that branch's
  children) and extended to any number of plugin panes rather than one.

## Non-goals

- **No `layout.toml`.** Only the synthesized default preset ships. There is no
  user-facing tree config, no schema to validate, and no new config file — so
  no new capability grant and nothing for a plugin to declare.
- **No visual change.** Geometry is identical at every terminal size for every
  combination of panels v1 could show. The ~115 pinned `insta` acceptance
  snapshots are the test: if one moves, the preset is wrong.
- **No tab-group leaf.** The central pane's Agent/Shell/Review tabs stay
  `CentralTab` state drawn over one region, as today. `tabs` becomes a leaf
  kind with `layout.toml`.
- **No `min_width`/`min_height` node keys.** Responsive collapse stays where v1
  put it — in the preset, which reads `two_panel_min_cols` /
  `three_panel_min_cols`. A per-node minimum has no consumer until the tree is
  user-editable, and this repo does not ship unused schema.
- **No anchors, no overlay layer, no interactive split resize.** All named as
  separate work by `docs/v2/` ADR-V23 and FEATURES-Layout.
- **No focus or visibility change.** `TogglePluginPane` (F10) still toggles the
  first declared pane and `Ctrl+L` still cycles onto the first focusable one.
  Seating N panes is a layout capability; giving each one a key is not part of
  it.

## Impact

- `src/session/workspace_tree.rs` (new — pure tree data, no crate-internal
  references, so `tests/architecture_rules.rs` needs no new edge).
- `src/ui/layout.rs` — `compute_layout` rewritten as preset + solver +
  projection; `PanelAreas::plugin_pane` becomes `plugin_panes`.
- `src/app/mod.rs` — `layout_for` passes a visible-pane count instead of a
  boolean.
- `src/app/view.rs` — `render_plugin_pane` draws every visible pane into its
  own region (behind `#[cfg(feature = "plugins")]`, as today).

**Feature gate**: the tree and its solver are **not** gated — they replace the
layout engine for every build. Plugin panes remain behind the existing
`plugins` Cargo feature: without it `App::visible_plugin_panes()` is `0`, the
preset emits no plugin leaf, and the solved tree is byte-identical to v1.
