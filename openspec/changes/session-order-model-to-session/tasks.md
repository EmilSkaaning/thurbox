# Tasks

## 1. Create the model's module

- [x] 1.1 Add `src/session/session_list.rs` and declare it in `src/session/mod.rs`.
- [x] 1.2 Move, unedited: `NO_REPO_GROUP`, `group_key`, `repo_set_key`, `group_display`,
  `repo_set_display`, `SessionOrder`, `compute_session_order`, `nest_group_members`,
  `sort_alphabetically_within_groups`, `sort_group_alphabetically`, `move_in_order`,
  `block_end`, `group_start`, `group_end`, `move_root_block_range`,
  `move_child_block_range`, `OrderedSessions`, `SessionMatch`, `SessionRow`, `RowInputs`,
  `resolve_rows`, `agent_status_text`.
- [x] 1.3 Rewrite their `crate::session::…` paths as `super::…`; the module may reference
  nothing outside `session` (`tests/architecture_rules.rs`).
- [x] 1.4 Move the unit tests that cover the moved functions with them.

## 2. Leave the drawing behind

- [x] 2.1 `src/ui/project_list.rs` imports the moved items from
  `crate::session::session_list` and keeps `SessionListItem`, `resolve_items`,
  `row_used_columns`, `fit_status_text`, `PendingSpawnSlot`, `pending_spawn_slot`,
  `SPINNER_MOTION_KEY`, the node builders, the style tables, `LeftPanelState`,
  `render_left_panel` and the pre-port span oracle.
- [x] 2.2 No `pub use` of a moved item — `migration/handover` refuses the re-export.

## 3. Update the callers

- [x] 3.1 `src/app/mod.rs`: `cached_session_order`, `session_order`,
  `move_active_session`, `sort_sessions_alphabetically`, `build_session_list_snapshot`.
- [x] 3.2 `src/app/view.rs`: `render_left_panel`'s order/matches/`OrderedSessions`, and
  `session_fuzzy`.
- [x] 3.3 `tests/bundled_session_list.rs` and `tests/session_list_pane_handover_gap.rs`.

## 4. Re-verdict the gate

- [x] 4.1 `the-module-is-the-kernels-model` → not blocked; probe re-derived from
  `src/session/session_list.rs` and from the absence of the calls in `src/app/mod.rs`.
- [x] 4.2 `the_module_is_the_kernels_navigation_not_only_the_panes_paint` asserts the new
  location, and that the coordinator no longer reaches into `ui` for any of the four.
- [x] 4.3 `no-pending-spawn-row` and `non-ascii-whitespace-is-the-kernels-trim`: probes
  follow the code they measure; both rows stay blocked.
- [x] 4.4 `the_verdict_is_derived_from_the_blockers` keeps refusing the handover, with
  `the-window-is-the-list-widgets` as the sole remaining structural row.
- [x] 4.5 `the_window_is_settled_before_what_depends_on_it` still holds.

## 5. Record it

- [x] 5.1 `docs/ARCHITECTURE.md`: an ADR for the relocation and the measured window gap.
- [x] 5.2 `docs/PHASE4-PANE-READINESS.md`: the section the gate's module doc points at.
- [x] 5.3 `CLAUDE.md`: the session-list ordering paragraph names the new module.

## 6. Verify

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --all-targets -- -D warnings`
- [x] 6.3 `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] 6.4 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 6.5 `cargo nextest run --all` — 2734 baseline, no snapshot moves
- [x] 6.6 `cargo nextest run --all --no-default-features`
- [x] 6.7 `cargo test --test teardown_gate`, `--test architecture_rules`,
  `--test session_list_pane_handover_gap`, `--test bundled_session_list`
- [x] 6.8 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] 6.9 Hand-drive: `scripts/dev/sandbox.sh`, confirm `Ctrl+J`/`Ctrl+K` navigation,
  `Shift+J`/`Shift+K` reorder across a group edge, `Shift+S` sort, repo grouping, nesting,
  and global search's session highlighting are unchanged.
