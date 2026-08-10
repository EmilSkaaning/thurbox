## 1. The tree data type

- [x] 1.1 Add `src/session/workspace_tree.rs` with `Axis`, `Sizing`
  (`Cells`/`Percent`/`Fill { min }`), `RegionId` (the ten kernel regions plus
  `Plugin(usize)`), `Region` (`Pane`/`Split`), `Node`, and the
  `Node::pane`/`Node::split` constructors. Register it in `src/session/mod.rs`.
  Pure data: no `crate::` references at all.
- [x] 1.2 Unit-test the constructors and `Node::region_ids` (every leaf, in
  order) in the same file.

**Verify:** `cargo nextest run -E 'test(workspace_tree)'`,
`cargo nextest run --test architecture_rules`

## 2. Solver

- [x] 2.1 Add `solve(&Node, Rect) -> Vec<(RegionId, Rect)>` to
  `src/ui/layout.rs`, mapping each child's `Sizing` onto the matching
  `ratatui::layout::Constraint` and recursing into sub-branches.
- [x] 2.2 Unit-test in `src/ui/layout.rs`: a nested split yields disjoint rects
  inside its parent; a fixed child keeps its extent as a fill sibling grows; a
  zero-cell child resolves to zero extent; solving is deterministic.

**Verify:** `cargo nextest run -E 'test(layout)'`

## 3. The default preset

- [x] 3.1 Replace `compute_layout`'s body in `src/ui/layout.rs` with
  `default_preset(area, params) -> Node`, `solve`, and a projection of the
  resulting `Placements`. Keep `split_vertical`'s band sizing, the
  `two_panel_min_cols` / `three_panel_min_cols` thresholds and
  `split_left_column`'s row maths as the preset's rules — emitting the same
  constraint lists, including the zero-length bands.
- [x] 3.2 Delete `three_panel_layout` / `two_panel_layout` / `left_column_split`
  and the `RightSlot` enum's rect-walking, now that the tree carries the order.

**Verify:** all 40 pre-existing tests in `src/ui/layout.rs` pass with their
expectations unmodified: `cargo nextest run -E 'test(layout)'`

## 4. Geometry is unchanged

- [x] 4.1 Run the acceptance suite: `cargo nextest run -E 'test(acceptance)'`
  and `cargo nextest run -E 'test(acceptance)' --features plugins`. The ~115
  pinned `insta` snapshots must not move. If one does, fix the preset.

**Verify:** `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`

## 5. N plugin panes

- [ ] 5.1 `src/ui/layout.rs`: `LayoutParams::show_plugin_pane: bool` →
  `plugin_panes: usize`; the preset emits one `RegionId::Plugin(i)` leaf per
  pane; `PanelAreas::plugin_pane: Option<Rect>` → `plugin_panes: Vec<Rect>`
  (dropping `Copy`).
- [ ] 5.2 Add the `CENTER_MIN_COLS` gate: drop trailing plugin leaves while the
  solved center is under it and more than one plugin leaf remains.
- [ ] 5.3 `src/app/mod.rs`: add `App::visible_plugin_panes() -> usize` (0 without
  the `plugins` feature) and pass it from `layout_for`.
- [ ] 5.4 `src/app/view.rs`: `render_plugin_pane` → `render_plugin_panes`, zipping
  the visible panes with `areas.plugin_panes`.
- [ ] 5.5 Tests in `src/ui/layout.rs`: two visible panes get two adjacent
  non-overlapping regions; hiding one leaves no gap and widens the center; a
  narrow terminal drops the trailing columns and keeps the center at the
  minimum; widening restores them; one pane is placed exactly where the single
  slot placed it. Update the one existing test that lists `LayoutParams` fields
  exhaustively.

**Verify:** `cargo nextest run -E 'test(layout)'`,
`GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`

## 6. Docs

- [ ] 6.1 Update the layout paragraph in `CLAUDE.md` (Architecture → `ui/`
  bullet) to describe the tree + preset instead of the slot list, and note that
  the right column seats every visible plugin pane.
- [ ] 6.2 Add the workspace tree to `docs/ARCHITECTURE.md` as an ADR with its
  rationale and the "preset reproduces v1" constraint.

**Verify:** `rumdl check .`

## 7. Close-out

- [ ] 7.1 `cargo fmt --all -- --check`; `cargo clippy --all-targets --features
  plugins -- -D warnings`; `cargo clippy --all-targets -- -D warnings`;
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`;
  `./scripts/dev/lint-luau.sh`; `rumdl check .`.
- [ ] 7.2 `cargo tree --edges normal | grep -c mlua` → `0`.
- [ ] 7.3 Both suites green against the 2035 / 2381 baseline.
