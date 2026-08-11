# Tasks

## 1. Re-verdict the three rows that stopped being requirements

- [x] `tests/file_viewer_pane_input_gap.rs`: `no-view-write` closes, with a probe that
      derives the route (the context is declarable, it maps to the pane's focus, and
      `focus_key_context` still answers that focus).
- [x] `no-filesystem-read` closes **on a conjunction**: the keyboard is declarable
      **and** `Capability::Files` is still narrow (no `fs` capability, no directory
      binding, no path on the published row). The row is now what keeps "the grant was
      unnecessary" from becoming "the grant happened".
- [x] `no-process-launch` closes the same way, keeping the assertion that no binding and
      no writer method reaches a process.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(file_viewer_pane_input_gap)'`.

## 2. Re-scope the two sub-mode rows

- [x] `sub-mode-keys-are-not-rebindable` and `no-query-write` become **properties** of
      the pane rather than handover blockers: the `/` sub-mode is kernel state before and
      after, so their probes stand and their `stands` says what they now mean.

## 3. Add the three decisions

- [x] `no-frame-node` becomes the **search bar's** row: seat chrome exists for one row
      (ADR-53) and this needs three plus a caret; probe both halves (the chrome hook
      exists, and it is a single row).
- [x] `no-file-viewer-seat`: `PaneSlot::seat()` names no `RegionId::FileViewer`.
- [x] `the-module-is-the-model-and-the-window`: promoted from a standalone test into the
      table, since it is now one of the three things that decides the verdict.
- [x] `the-column-has-a-second-kernel-occupant`: `layout_for` force-shows the column for
      a review and `render_file_viewer` draws the review's list into it.

## 4. The verdict follows from the rows, and characterises the remainder

- [x] `the_verdict_is_derived_from_the_blockers`: still "not portable", and now asserts
      that **nothing outstanding is structural** — the headline.
- [x] The module note is rewritten: what this gate measures now, and that the expected
      capability widening turned out to be **none**.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and
      `--no-default-features`.

## 5. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-54 — the refusal, the three decisions, and the
      capability that was not needed.
- [x] `docs/PHASE4-PANE-READINESS.md` §29.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the file-viewer row's reason.
- [x] Verify: `rumdl check .`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
      --all-features`.

## 6. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `openspec validate file-viewer-handover-verdict --strict`
