# Tasks

## 1. Publish the cursor as a scroll anchor

- [x] `src/session/pane_context.rs`: add `TasksSnapshot::cursor: Option<usize>`,
      documenting the anchor-versus-appearance split and why the anchor is not
      focus-gated.
- [x] `src/app/mod.rs` (`build_tasks_snapshot`): publish the cursor clamped to the
      published rows, `None` when the list is empty or the cursor falls past
      `MAX_TASK_ROWS`; leave the per-row `selected` gate untouched.
- [x] `src/plugin/kernel_state.rs` (`tasks_table`): expose it one-based as
      `cursor`, matching the review section's spelling, via the existing
      `set_opt`.
- [x] Verify: `cargo nextest run --features plugins -E 'test(kernel_state)'` and
      `test(pane_context)`.

## 2. Let both panes window through one implementation

- [x] `src/ui/tasks_panel.rs`: replace `visible_rows` with a row builder that fits
      **every** entry, have `tasks_tree` take the cursor and build a selectable
      list, and call `ui::file_viewer::visible_window` once more for the click
      hitboxes — the shape `ui::file_viewer::render` already uses.
- [x] `src/plugin/bundled/thurbox.d.luau`: `TaskList` gains `cursor: number?`.
- [x] `src/plugin/bundled/tasks/init.luau`: hand the cursor to `ui.list`.
- [x] Verify: `cargo nextest run -E 'test(tasks_panel)'`, and
      `git status --short src/app/snapshots/` is empty — the native pane's frames
      must not move.

## 3. Re-aim the port's oracle at the closed gap

- [x] `tests/bundled_tasks_panel.rs`: publish every row plus the cursor (not the
      windowed rows), keep the title-fitting divergence, and replace
      `a_list_longer_than_the_pane_is_windowed_by_the_kernel_only` with a
      **frame**-equality test at a size where the pane scrolls, mirroring
      `tests/bundled_file_viewer.rs`.
- [x] Verify: `cargo nextest run --features plugins -E 'test(bundled_tasks)'`.

## 4. Gate the input verdict

- [x] `tests/tasks_pane_input_gap.rs` (new): one blocker per host power the pane's
      keys need — the view write, the disjointness of the input and cursor paths,
      the create/text write, the central seat, the modal, and the PTY/spawn reach
      — each re-derived from the source, each tagged structural or vocabulary, and
      a test asserting a record write is not the write these keys need.
- [x] Assert the bundled plugin declares no `input` capability and no
      keybinding, so the verdict and the shipped plugin cannot disagree.
- [x] Verify: `cargo test --test tasks_pane_input_gap` and
      `cargo test --test tasks_pane_input_gap --features plugins` (identical in
      both, like the teardown gate).

## 5. Record it

- [x] `docs/PHASE4-PANE-READINESS.md`: §15 — the attempt, the key-by-key table,
      the disjointness finding, what closed, and what stays kernel.
- [x] `docs/ARCHITECTURE.md`: ADR-38 — the anchor/appearance split and the input
      verdict, with the rejected alternatives.
- [x] Verify: `rumdl check .`

## 6. Whole-tree verification before commit

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
      — the `tasks-plugin` row must still be blocked.
- [x] `./scripts/dev/lint-luau.sh`
- [x] Drive the real thing: `scripts/dev/sandbox.sh --fresh --plugins --show tasks`,
      create more tasks than the pane has rows, and confirm with `tmux send-keys`
      that the native pane's keys still work and that the plugin's copy scrolls
      with the cursor.
