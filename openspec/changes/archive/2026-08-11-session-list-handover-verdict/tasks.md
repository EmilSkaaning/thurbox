# Tasks

## 1. Re-verdict the three rows ADR-51's route retires

- [x] `tests/session_list_pane_handover_gap.rs`: `scoped-keys-silenced-by-the-handover`,
      `no-active-session-write` and `no-session-record-write` become `blocked: false`,
      each on a **conjunction** — the route is declarable, maps to
      `InputFocus::SessionList`, is still resolved by `focus_key_context`, **and** the
      power the row named is still absent. Each `stands` says which half is which.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test
      session_list_pane_handover_gap`.

## 2. Promote the oracle's two remaining divergences into rows

- [x] `tests/session_list_pane_handover_gap.rs`: `the-window-is-the-list-widgets`
      (structural), probing all four behaviours derived from `ListState::offset()` —
      the visible rows, the border indicators, the hitboxes, the pending-spawn slot.
- [x] `tests/session_list_pane_handover_gap.rs`:
      `non-ascii-whitespace-is-the-kernels-trim` (vocabulary), probing that the kernel
      trims with `str::trim` and the plugin with Luau's `%s`.
- [x] `tests/bundled_session_list.rs`: each divergence's doc names its gate row, so the
      measurement and the verdict point at each other.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_session_list) or binary(session_list_pane_handover_gap)'`.

## 3. The verdict, the wrap and the ordering

- [x] `the_verdict_is_derived_from_the_blockers`: the deciders become
      `the-window-is-the-list-widgets` and `the-module-is-the-kernels-model`, and the
      three closed rows are asserted **not** structural blockers.
- [x] A new rule asserting the wrap is not a blocker: both ends are kernel focuses
      (`focus_for_keyboard`) and the condition is the pane's presence
      (`automations_pane_provided`), pointing at ADR-56.
- [x] A new rule asserting the ordering: the window is settled before the module and
      before the two rows derived from it.
- [x] Rewrite the module note: the finding is no longer the focus wall.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test
      session_list_pane_handover_gap` and `--test teardown_gate`.

## 4. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-57 — the refusal, the four behaviours off the widget,
      and why three rows close without a grant.
- [x] `docs/PHASE4-PANE-READINESS.md` §32.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the session list's remaining rows.
- [x] Verify: `rumdl check .`.

## 5. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] `openspec validate session-list-handover-verdict --strict`
- [x] Hand-drive the pane that is **still native**, to confirm nothing moved: the
      session list draws with its repo-group header, `Ctrl+J`/`Ctrl+K` navigate, and the
      left column's circular list works in both directions — `j` at the last session
      enters the band, `j` past the last automation loops to the top of the list, `k` at
      the first automation returns to the last session. (`Ctrl+J` is `NextSession`, a
      global chord bound to `switch_session_forward`, and never wrapped into the band;
      verified identical at the previous commit so the handover did not change it.)
