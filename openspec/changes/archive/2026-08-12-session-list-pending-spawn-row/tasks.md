# Tasks

## 1. The model

- [x] 1.1 `src/session/session_list.rs`: `PendingSpawnSlot` + `pending_spawn_slot` move in
  from `src/ui/project_list.rs`, unchanged, beside `compute_session_order`; `PendingRow`
  joins `SessionRow`.
- [x] 1.2 `src/session/session_list.rs` tests: the moved unit tests, plus one asserting the
  slot lands where `compute_session_order` will put the real row.

Verify: `cargo nextest run -E 'test(pending_spawn_slot)'`.

## 2. The pane builds it

- [x] 2.1 `src/ui/project_list.rs`: `SessionListItem::Pending(PendingRow)`; `resolve_items`
  inserts it (and its header) at the resolved slot; `session_list_tree` builds it.
- [x] 2.2 `src/ui/project_list.rs`: `pending_spawn_node` declares `Motion::cycle` on the
  glyph while the phase is working, keyed `pending`, and the pane's frame table carries
  that key.
- [x] 2.3 `src/ui/project_list.rs`: `render_session_section` loses its own insertion and
  finds the placeholder by scanning the items; hitboxes unchanged.
- [x] 2.4 `src/ui/project_list.rs` tests: the tree carries the placeholder in its group,
  a new group brings its own header, and the placeholder gets no hitbox.

Verify: `cargo nextest run -E 'test(project_list)'`.

## 3. The publication

- [x] 3.1 `src/session/pane_context.rs`: `SessionRowSnapshot.pending_phase: Option<String>`.
- [x] 3.2 `src/app/mod.rs`: `build_session_list_snapshot` inserts the placeholder at the
  model's slot, carrying the group label when it opens one.
- [x] 3.3 `src/plugin/kernel_state.rs`: `pendingPhase` crosses, absent on a session's row.
- [x] 3.4 `src/plugin/bundled/thurbox.d.luau`: declare it.
- [x] 3.5 `src/plugin/kernel_state.rs` tests: a placeholder crosses with its phase and its
  group; a session's row carries no phase.

Verify: `cargo nextest run -E 'test(kernel_state) or test(session_list)'`.

## 4. The plugin draws it

- [x] 4.1 `src/plugin/bundled/session-list/init.luau`: a pending row is a distinct node —
  spinner or `◌`, the label in `secondary`, the phase in `muted`.
- [x] 4.2 `tests/bundled_session_list.rs`: two recorded cases — a spawn joining an existing
  group, and one opening its own — each asserting `native == recording` and
  `plugin == native`.

Verify: `cargo nextest run --test bundled_session_list`;
`./scripts/dev/lint-luau.sh`.

## 5. Close the gate row

- [x] 5.1 `tests/session_list_pane_handover_gap.rs`: `no-pending-spawn-row` → **met**, with
  a probe asserting both halves (the row is published, the slot left the renderer).
- [x] 5.2 `the_verdict_is_derived_from_the_blockers` now finds **no** outstanding row, so
  the assertion is **inverted** rather than deferred: the gate records that the pane is
  portable and fails if a row reopens. The gate itself is retired by the handover.

Verify: `cargo test --test session_list_pane_handover_gap`.

## 6. Docs

- [x] 6.1 `docs/ARCHITECTURE.md`: ADR for the published placeholder and the declared
  spinner.
- [x] 6.2 `docs/PHASE4-PANE-READINESS.md`: record the last row closing.

Verify: `rumdl check .`

## 7. Full gate

- [x] 7.1 `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo clippy --all-targets --no-default-features -- -D warnings`;
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`.
- [x] 7.2 `cargo nextest run --all` and `cargo nextest run --all --no-default-features`.
- [x] 7.3 `cargo test --test teardown_gate`; `cargo test --test architecture_rules`;
  `./scripts/dev/lint-luau.sh`; `./scripts/dev/lint-workflows.sh`; `rumdl check .`
- [x] 7.4 Hand-drive a spawn in `scripts/dev/sandbox.sh` and watch the placeholder.
