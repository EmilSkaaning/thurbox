# Tasks

## 1. The publication carries the tree

- [x] 1.1 `ReviewFileRowSnapshot` (folder / file) and `MAX_REVIEW_FILE_ROWS` in
      `src/session/pane_context.rs`; `ReviewSnapshot` gains `file_rows` + `file_cursor`.
- [x] 1.2 `CodeReviewState::file_row_snapshots(limit)` builds them from
      `session::review::file_tree_rows`, with `snapshot_file_rows()` as the
      publication's bounded call (`src/app/code_review.rs`).
- [x] 1.3 `App::build_review_snapshot` publishes both (`src/app/mod.rs`).
- [x] 1.4 `plugin::kernel_state::review_table` exposes `files` + `fileCursor`, one-based
      (`src/plugin/kernel_state.rs`); `ReviewFileRow`/`ReviewDiff` declared in
      `src/plugin/bundled/thurbox.d.luau`.
- [x] Verify: `cargo nextest run --all --no-default-features`

## 2. The kernel's builder takes the published rows

- [x] 2.1 `ui::code_review::files_list_tree(rows, cursor)`, with the row builder taking
      a `FileRow` struct rather than the state (`src/ui/code_review.rs`).
- [x] 2.2 `render_files_list` calls `file_row_snapshots(usize::MAX)` and maps a clicked
      row's path back to the review's own file index (`src/ui/code_review.rs`).
- [x] Verify: `cargo nextest run -E 'test(changed_files)'` — the pre-port span oracle
      still holds cell for cell.

## 3. The bundled plugin's second pane

- [x] 3.1 `[[panes]] id = "files"` in `src/plugin/bundled/code-review/plugin.toml`,
      right column, hidden, no seat and no keyboard.
- [x] 3.2 `filesPane` in `src/plugin/bundled/code-review/init.luau`, reusing the file
      header's `fileStatus` table so the two rows cannot disagree about a rename.
- [x] Verify: `./scripts/dev/lint-luau.sh`

## 4. The oracle

- [x] 4.1 `tests/bundled_review_files.rs`: tree equality against the kernel builder,
      seven recorded cases, both row kinds and all four statuses asserted covered.
- [x] 4.2 The manifest's two absences asserted (no seat, no keyboard) so the port
      cannot become a handover by omission.
- [x] Verify: `cargo nextest run --test bundled_review_files`

## 5. The record

- [x] 5.1 ADR in `docs/ARCHITECTURE.md`, section in `docs/PHASE4-PANE-READINESS.md`.
- [x] Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo clippy --all-targets --no-default-features -- -D warnings`,
      `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `cargo nextest run --all`, `cargo test --test teardown_gate`,
      `cargo test --test architecture_rules`,
      `cargo test --test code_review_pane_handover_gap`, `rumdl check .`
