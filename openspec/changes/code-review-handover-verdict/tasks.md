# Tasks

## 1. The gate

- [x] `tests/code_review_pane_handover_gap.rs` (new): one `Blocker` per requirement
      the handover needs and does not have — the two seats, the capture-keyed
      keyboard, the review write, the retarget, the export pair, the cursor, the
      resolved width, the mouse column, the anchored overlay and the in-pane text
      field — each tagged structural / vocabulary / wiring, each probed from the
      declaration it is about.
- [x] `recorded_blockers_match_the_tree` and `the_verdict_is_derived_from_the_blockers`,
      the latter asserting both directions.
- [x] Verify: `cargo test --test code_review_pane_handover_gap` and the same
      `--features plugins` — identical in both, like the teardown gate.

## 2. The findings, as their own tests

- [x] `the_review_is_two_seats_not_one`: names what each seat draws, that the
      file-viewer column is forced present by an open review, that the second seat has
      its own focus and keys, and that a plugin is offered one column.
- [x] `the_reviews_keyboard_resolves_no_action`: no review `KeyContext`, both captures
      ahead of `lookup_in`, `focus_key_context` naming no plugin pane — and that the
      one bindable review action is the toggle that *opens* the view.
- [x] `the_mouse_channel_carries_a_row_where_the_pane_needs_a_column`: the row-only
      click, the target kinds the pane has, and `cr_click_row`'s `rel_x`/`width`.
- [x] `the_review_cursor_is_a_narrower_grant_than_the_session_lists`: both cursors,
      what reads each, and that neither is writable.
- [x] `the_native_pane_is_still_what_thurbox_draws`, plus the plugin still declaring
      exactly two capabilities.

## 3. Non-vacuity, by perturbation

- [x] Add `PaneSlot::Central` → both seat rows flip and the gate names them; revert.
- [x] Add default-bodied `set_review_mark` + `set_review_cursor` to `KernelWriter` →
      `no-review-write` and `no-cursor-write` flip; revert.
- [x] Add `KeyContext::CodeReview` → the keyboard row flips; revert.
- [x] `git diff` clean after each.

## 4. Record it

- [x] `docs/PHASE4-PANE-READINESS.md` §20: the attempt, the eleven rows, the three
      findings, and the ordering.
- [x] `docs/ARCHITECTURE.md` ADR-45: the verdict with its rejected alternatives.
- [x] Verify: `rumdl check .`

## 5. Whole-tree verification before commit

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — the
      `code-review-plugin` row must still be blocked, for the same reason as before.
- [x] `./scripts/dev/lint-luau.sh`; `./scripts/dev/lint-workflows.sh`; `rumdl check .`
- [x] `openspec validate --all --strict`
- [x] Drive the real thing: with the plugin pane visible beside the native review,
      confirm by hand that `Ctrl+L` never focuses the reproduction (it declares no
      `input`, so `PluginPane::is_focusable` is false) while every review key still
      reaches the native pane.
