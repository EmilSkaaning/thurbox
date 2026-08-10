## 1. Params struct

- [x] 1.1 Add `LayoutParams` (named fields + `Default`) to `src/ui/layout.rs` and change `compute_layout` from 9 positional arguments to `(area, LayoutParams)`.
- [x] 1.2 Convert `App::layout_for` in `src/app/mod.rs` to build the struct.
- [x] 1.3 Convert all 35 test call sites in `src/ui/layout.rs`, omitting fields that were `false`/`0` so each test states only what it exercises.

**Verify:** `cargo check --all-targets`

## 2. Ordered column occupants

- [x] 2.1 Add a private `RightSlot` enum and `LayoutParams::right_slots()` returning the column's occupants in draw order.
- [x] 2.2 Build the right column's constraints from that list and assign rects by walking it, so a hidden occupant leaves no gap and a new one is a list entry.
- [x] 2.3 Add `PanelAreas::plugin_pane` and derive `Debug, Clone, Copy, PartialEq, Eq` so layouts can be compared in tests.

**Verify:** `cargo nextest run -E 'test(layout)'`

## 3. Geometry is unchanged

- [x] 3.1 All 34 pre-existing layout tests pass unmodified in behavior.
- [x] 3.2 All 115 acceptance tests pass, including the pinned `insta` snapshots — the strongest evidence that no rendered frame moved.
- [x] 3.3 New tests: default params show nothing; omitting a panel equals disabling it explicitly.

**Verify:** `cargo nextest run -E 'test(acceptance)'`, `cargo nextest run -E 'test(layout)'`

## 4. Close-out

- [x] 4.1 `cargo nextest run --all` → 1985 passed. `--features plugins` → 2131 passed. Clippy clean in both configurations, rustdoc/rumdl/fmt clean.
- [x] 4.2 One assertion in a new test was wrong about the mechanism and was corrected rather than loosened: hiding an occupant does **not** slide the remaining panes left, because the terminal holds the `Min(0)` slot and absorbs the freed width. The test now asserts that (no gap, terminal grows, pane keeps its width).
