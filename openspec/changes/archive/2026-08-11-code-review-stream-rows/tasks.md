# Tasks

## 1. Two style tokens for the palette's diff colours

- [x] `src/session/view_tree.rs`: add `StyleToken::DiffAdded` / `DiffRemoved` with
      wire names `diff_added` / `diff_removed`, documenting why `Added` is not
      reused.
- [x] `src/ui/plugin_pane.rs` (`token_color`): resolve them to `palette.diff_added`
      / `palette.diff_removed`.
- [x] `src/plugin/bundled/thurbox.d.luau`: extend `StyleToken`.
- [x] Verify: `cargo nextest run -E 'test(view_tree)'` and `test(plugin_pane)`.

## 2. The review section publishes rows

- [x] `src/session/pane_context.rs`: `ReviewSnapshot::lines` → `rows:
      Vec<ReviewRowSnapshot>` with the six kinds, each documented for what it
      carries and what it deliberately does not; keep `cursor` and `number_width`;
      restate `MAX_REVIEW_ROWS` as a bound over every kind.
- [x] `src/app/code_review.rs`: `CodeReviewState::snapshot_rows()` — the one pure
      extraction, applying the bound and dropping a cursor past it.
- [x] `src/app/mod.rs` (`build_review_snapshot`): call it.
- [x] `src/plugin/kernel_state.rs` (`review_table`): one Luau table per row kind,
      tagged `row`, with the line's `kind` unchanged.
- [x] Verify: `cargo nextest run --features plugins -E 'test(kernel_state)'` and
      `test(pane_context)`.

## 3. The kernel's builder covers every row kind

- [x] `src/ui/code_review.rs`: `review_stream_tree` over snapshot rows, plus one
      builder per kind; keep `diff_row_tree` as the line kind's builder.
- [x] Pin each new kind to the untouched native renderer by painting both
      (`assert_same_row`), across: each file status, folded/unfolded,
      reviewed/unreviewed, a wide `@@` range, each classification, a multi-line
      comment body, the summary header, an informational row, and the cursor on
      each selectable kind.
- [x] Assert the enumerated divergence directly: a row that fits is identical, a row
      that overflows clips where the native ellipsizes.
- [x] Verify: `cargo nextest run -E 'test(code_review)'`, and
      `git status --short src/app/snapshots/` empty — the native paint path is
      untouched.

## 4. The plugin draws the document

- [x] `src/plugin/bundled/code-review/init.luau`: dispatch over `row`, one builder
      per kind, gutter and highlighter unchanged.
- [x] `src/plugin/bundled/code-review/plugin.toml`: re-state the scope — what is now
      drawn, and that the remainder is behaviour.
- [x] `src/plugin/bundled/thurbox.d.luau`: `ReviewRow` union and `ReviewDiff.rows`.
- [x] Verify: `./scripts/dev/lint-luau.sh`.

## 5. Re-aim the port's oracle

- [x] `tests/bundled_code_review.rs`: cases covering every row kind; keep the
      node-budget measurement; rewrite
      `the_out_of_scope_surface_is_absent_rather_than_approximated` to assert only
      what is *still* absent (the find bar, the target picker, the compose box, the
      footer) and to assert the newly drawn rows are **present**.
- [x] Verify: `cargo nextest run --features plugins -E 'test(bundled_code_review)'`.

## 6. Record it

- [x] `docs/PHASE4-PANE-READINESS.md` §11: the document/behaviour split, what closed,
      and the remainder.
- [x] `docs/ARCHITECTURE.md`: an ADR for the published-row model, the keybinding-text
      exception and the clip-versus-ellipsis divergence.
- [x] Verify: `rumdl check .`

## 7. Whole-tree verification before commit

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — the
      `code-review-plugin` row must still be blocked.
- [x] `./scripts/dev/lint-luau.sh`; `./scripts/dev/lint-workflows.sh`; `rumdl check .`
- [x] Drive the real thing: `scripts/dev/sandbox.sh --fresh --plugins --show
      code-review`, open a review over a worktree with a committed diff, and confirm
      with `tmux send-keys` that the plugin's copy shows the same headers, marks and
      comments as the native view and follows its cursor.
