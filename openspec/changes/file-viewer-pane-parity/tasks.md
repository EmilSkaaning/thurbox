# Tasks

## 1. The node

- [x] 1.1 `src/session/view_tree.rs`: add the scroll-track field to
  `ViewNode::List` plus a constructor that declares one; document why the field
  is not inferred from the cursor. Update the exhaustive matches
  (`src/ui/plugin_pane.rs`, `src/ui/project_list.rs`, `src/ui/code_review.rs`,
  `src/ui/tasks_panel.rs`).
- [x] 1.2 `src/plugin/view.rs`: convert the declaration, refusing a non-boolean
  as a named bad field; unit-test the accept and the refusal.
- [x] 1.3 `src/plugin/capabilities.rs`: third argument on the granted `ui.list`.
- [x] 1.4 `src/plugin/bundled/thurbox.d.luau`: the declared signature.
- [x] Verify: `cargo nextest run --features plugins -E 'test(view_tree) or test(convert)'`

## 2. Drawing it in one place

- [x] 2.1 `src/ui/scrollbar.rs`: `draw_into` (buffer) and `geom_for` (no draw);
  `render_into` becomes a `Frame` wrapper over both.
- [x] 2.2 `src/ui/plugin_pane.rs`: a list declaring a track reserves the
  rightmost column via `scrollbar::reserve_track`, windows and paints the rows in
  what is left, draws the thumb, and derives its row hitboxes from the narrowed
  rect.
- [x] 2.3 Unit tests in `src/ui/plugin_pane.rs`: an overflowing list with a track
  reserves a column and draws a thumb; the same list without the declaration does
  not; a click on the track resolves to no row.
- [x] Verify: `cargo nextest run -E 'test(plugin_pane) or test(scrollbar)'`

## 3. The native pane

- [x] 3.1 `src/ui/file_viewer.rs`: `file_tree` declares the track for a populated
  tree; `render_rows` paints into the whole list area and keeps `reserve_track`
  for the hitboxes and the recorded geometry via `geom_for`.
- [x] 3.2 Verify the native frames did not move:
  `cargo nextest run -E 'test(file_viewer)'` (its own frame-equality tests against
  the retained legacy renderer) and
  `cargo nextest run -E 'test(acceptance)'` (no snapshot moves).

## 4. The plugin

- [x] 4.1 `src/plugin/bundled/file-viewer/init.luau`: declare the track, with the
  comment explaining that the column is the kernel's.
- [x] 4.2 `tests/bundled_file_viewer.rs`: replace
  `the_scrollbar_is_the_native_panes_chrome_only` with a frame-equality assertion
  including the track's column, plus a non-vacuity check against a render without
  the declaration.
- [x] Verify: `cargo nextest run --features plugins -E 'test(bundled_file_viewer)'`
  and `./scripts/dev/lint-luau.sh`

## 5. The gate

- [x] 5.1 `tests/file_viewer_pane_input_gap.rs`: one probe per blocker — the view
  write (distinguished from a record write), the filesystem read behind an
  expansion, the process launch behind opening a file, the sub-mode's fixed keys,
  the inert plugin track, and the model living in the module a handover deletes.
  Plus the standing assertions: the plugin declares no `input` and no binding, and
  `src/app/view.rs` still draws the native pane.
- [x] Verify: `cargo nextest run --features plugins -E 'test(file_viewer_pane_input_gap)'`

## 6. The record

- [x] 6.1 `docs/ARCHITECTURE.md`: ADR-39 — where a scroll track is reserved, and
  why this pane's keys are not portable.
- [x] 6.2 `docs/PHASE4-PANE-READINESS.md` §16: the attempt, what landed, the four
  input blockers, and the capability that did not grow.
- [x] 6.3 `docs/PHASE6-TEARDOWN-READINESS.md`: the worklist gains this pane's
  blockers, including that its module is its model.
- [x] Verify: `rumdl check .`

## 7. Whole-tree verification

- [x] 7.1 `cargo fmt --all -- --check`
- [x] 7.2 `cargo clippy --all-targets --features plugins -- -D warnings` and
  `cargo clippy --all-targets -- -D warnings`
- [x] 7.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 7.4 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and
  `… --all --features plugins`
- [x] 7.5 `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
- [x] 7.6 Hands-on: `scripts/dev/sandbox.sh --fresh --plugins --show files`, drive
  the native pane's keys with `tmux send-keys`, and confirm the plugin's copy
  scrolls and grows a thumb in the same column.
