# Tasks

## 1. The folder tree becomes a model

- [x] 1.1 Move `build_file_tree`/`TreeRow` from `src/ui/code_review.rs` to
      `src/session/review.rs` as `file_tree_rows`/`FileTreeRow`, unchanged in behaviour,
      with the doc stating why the pure-data layer owns it.
- [x] 1.2 Move `file_tree_groups_dirs_and_keeps_indices` to `src/session/review.rs`'s test
      module, with the model it tests.
- [x] Verify: `cargo nextest run -E 'test(file_tree_groups)'`

## 2. The pane paints through the shared painter

- [x] 2.1 `ui::code_review::files_list_tree` builds the list: one child per tree row,
      `TextStyle::selected` on the current file's row, the anchor named so the kernel
      windows it (`src/ui/code_review.rs`).
- [x] 2.2 `render_files_list` draws its block and legend as before, then paints the tree
      into what is left through `ui::plugin_pane::render_tree_rows`; hitboxes come out of
      the paint, filtered to file rows (`src/ui/code_review.rs`).
- [x] 2.3 `status_token` names the status glyph's colour role once; `status_color`
      resolves it for the diff's own header row (`src/ui/code_review.rs`).
- [x] Verify: `cargo nextest run -E 'test(changed_files)'`

## 3. The evidence

- [x] 3.1 Retain the pre-port span builders in the test module and assert the two paints
      are buffer-equal at widths 30 and 12 (`src/ui/code_review.rs`).
- [x] 3.2 Pin the stated behaviour change: twelve files, five rows, cursor on the tenth —
      third of five, clamped at the tail (`src/ui/code_review.rs`).
- [x] Verify: `cargo nextest run --all` and `cargo nextest run --all --no-default-features`

## 4. The record

- [x] 4.1 ADR in `docs/ARCHITECTURE.md` and a section in
      `docs/PHASE4-PANE-READINESS.md`, naming the behaviour that changed and that no pane
      was handed over.
- [x] Verify: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo clippy --all-targets --no-default-features -- -D warnings`,
      `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `cargo test --test teardown_gate`, `cargo test --test architecture_rules`,
      `rumdl check .`
