# Tasks

## 1. The painter reports the window it drew

- [x] 1.1 `ui::plugin_pane::render_tree_rows` returns the outermost list's clipped-row
      counts alongside its hitboxes, collected in `RowSink` next to the rects and claimed
      by the same rule (outermost list, once).
- [x] 1.2 Update the one existing caller (`App::paint_plugin_pane`) to take the rows out
      of the new return value; nothing consumes the counts yet.

## 2. Clipped-row indicators become shared vocabulary

- [x] 2.1 Move the `▲ N` / `▼ N` painter to `src/ui/mod.rs` as
      `draw_clipped_indicators(buf, block_area, above, below)`, taking a buffer so both a
      native pane and the plugin-pane painter can call it.
- [x] 2.2 Delete `render_scroll_indicators_variable` and `visible_count_from_heights` from
      the pane — both existed only to re-derive a window the painter now reports.

## 3. A header and the row it heads become one item

- [x] 3.1 `ui::project_list::session_list_tree` emits a `Column` of `[header, row]` where
      it emitted two flat children; `selected` counts items.
- [x] 3.2 The bundled plugin emits `ui.column({header, row})` for the same rows, and its
      cursor index counts children the same way.
- [x] 3.3 Re-record `tests/snapshots/bundled_session_list__*.snap` from the **native**
      tree, which is still present.

## 4. The native pane windows by the kernel's rule

- [x] 4.1 `render_session_section` draws its block, then paints the list tree into the
      block's inner rect through `render_tree_rows`; hitboxes come from the return value,
      indicators from its counts.
- [x] 4.2 The pending-spawn placeholder is inserted into the folded items at
      `pending_spawn_slot`'s index, and still carries no hitbox.
- [x] 4.3 Delete `App::session_list_state` and the `ListState` field on `LeftPanelState`.
- [x] 4.4 The pane's own hitbox test keeps its claim: a two-line item is one hitbox
      spanning both lines.

## 5. The divergence closes

- [x] 5.1 Replace `the_two_panes_window_a_long_list_by_different_rules` with its opposite:
      at a height where the list overflows, the drawn slice and the clipped counts are the
      same for both panes.
- [x] 5.2 Flip `the-window-is-the-list-widgets` to closed in
      `tests/session_list_pane_handover_gap.rs`, rewriting `stands` and the probe to
      re-derive the convergence rather than the widget.
- [x] 5.3 `the_verdict_is_derived_from_the_blockers` records that no structural row is
      left, and that the remaining rows are vocabulary.
- [x] 5.4 `the_window_is_settled_before_what_depends_on_it` keeps its ordering claim over
      the table and drops the half that required the decider to be outstanding.

## 6. Documentation

- [x] 6.1 ADR for the convergence: the direction, the rejected alternatives, the
      behaviour that changed.
- [x] 6.2 `docs/PHASE4-PANE-READINESS.md` section recording the row closed and what is
      left.

## 7. Verification

- [x] 7.1 `cargo fmt --all -- --check`
- [x] 7.2 `cargo clippy --all-targets -- -D warnings` and the `--no-default-features` form
- [x] 7.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 7.4 `cargo nextest run --all` and the `--no-default-features` form
- [x] 7.5 `cargo test --test teardown_gate`, `--test architecture_rules`
- [x] 7.6 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] 7.7 Hand-drive the sandbox: `j`/`k`, `Ctrl+J`/`Ctrl+K`, `Shift+J`/`Shift+K`,
      `Shift+S`, a click on a group header, and an overflowing list.
