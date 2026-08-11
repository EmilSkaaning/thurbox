# Tasks

## 1. The bundled pane binds itself to the pane's identity

- [x] `src/plugin/bundled/info-panel/plugin.toml`: `title = "Info"` (the native
      pane's title — it is no longer a copy beside it), `toggle_action =
      "ToggleInfoPanel"`, `feature = "info_panel"`. `default_visible = false`
      stays, with the reason in the comment.
- [x] `src/plugin/bundled/info-panel/init.luau`: rewrite the header comment — it
      describes a reproduction of `ui::info_panel::info_tree`, which no longer
      exists. It is the pane.
- [x] `tests/bundled_manifests.rs`: add `("info-panel", "info")` to
      `PANES_DRAWN_IN_A_NATIVE_PANES_PLACE` and rewrite its doc — the list is no
      longer empty and the seed is still `false`.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_manifests)'`.

## 2. Relocate what the pane declared and others own

- [x] `src/app/metrics_state.rs`: declare `SystemMetrics` here (the collector fills
      it, `MetricsState` owns it), with a doc saying why it is not in `session`.
- [x] `src/app/mod.rs`: `collect_system_metrics` and the tests that build one use
      the new path.
- [x] `AutomationEntry` is **deleted**, not moved: it carried a pre-rendered
      countdown string into the native pane, and the published snapshot carries
      seconds.
- [x] Verify: `cargo check --all`.

## 3. Stop drawing it, and delete the kernel's occupant of the seat

- [x] `src/app/view.rs`: delete `App::render_info_panel` and its call; drop
      `info_panel` from the `use crate::ui::{…}` list. Update
      `render_plugin_panes`' doc, which names it among the four guards.
- [x] `src/app/mod.rs`: delete the `show_info_panel` field, its initialiser, the
      `enforce_feature_visibility` branch and the `handle_resize` branch (the
      layout's own ≥120 rule is what hides the seat now). `layout_for`'s
      `show_info_panel` becomes `self.seat_taken(PaneSlot::CenterLeft)`.
- [x] `src/app/key_handlers.rs`: `Action::ToggleInfoPanel` keeps its
      `[features] info_panel` gate and, when no pane claims the seat, reports which
      plugin provides the pane instead of doing nothing.
- [x] `src/app/mod.rs`: `App::pane_bound_to(action)` (and its
      `#[cfg(not(feature = "plugins"))]` `false` stub) so the arm above asks one
      question in one place.
- [x] Verify: `cargo check --all` and `cargo check --all --no-default-features`.

## 4. A pane that cannot receive input records no click target

- [x] `src/app/view.rs`: in `render_plugin_panes`, record row and whole-rect click
      targets only for a pane that accepts input. Comment the reason: both handlers
      already refuse such a pane, so the target's only effect was to consume the
      click — which is what would have eaten drag-select in the Info column.
- [x] Test: a visible plugin pane with no `input` records no click target, and one
      with `input` still records rows plus its rect.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(plugin_pane)'`.

## 5. Delete the native renderer

- [x] `git rm src/ui/info_panel.rs`; drop `pub mod info_panel;` from
      `src/ui/mod.rs`.
- [x] Fix the doc comments that point at it: `src/ui/mod.rs`,
      `src/ui/tasks_panel.rs`, `src/ui/file_viewer.rs`, `src/ui/plugin_pane.rs`,
      `src/session/mod.rs`, `src/app/mod.rs`.
- [x] Verify: `cargo check --all`, `cargo check --all --no-default-features`,
      `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`.

## 6. The oracle, rewritten against its recording

- [x] `tests/bundled_info_panel.rs`: `Case` carries `SystemSnapshot` and
      `Vec<UpcomingAutomationSnapshot>` instead of the deleted types; drop
      `native_tree`, `countdown_secs` and the two assertions that named
      `info_tree`; keep the snapshot assertion as the expectation. Rewrite the
      module doc: the recording is now the only edge, which is what ADR-42 was for.
- [x] The ten `tests/snapshots/bundled_info_panel__*.snap` files MUST NOT
      change. Confirm with `git status` — a moved recording means the published
      context moved, which is a bug in this change, not a snapshot to accept.
- [x] Non-vacuity: perturb the plugin (one row) and observe the oracle fail;
      revert. Record the observed failure in this file.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_info_panel)'`.

## 7. The gate: re-verdict the row and move its examples

- [x] `tests/teardown_gate.rs`: `info-panel-plugin` → `ready: true`.
- [x] `a_reproduced_pane_is_not_a_replaced_one`: retarget to the tasks pane, which
      is still native, and say in the doc why the example must name a still-native
      pane.
- [x] `every_pane_row_names_its_native_renderer`: exempt a row whose verdict is
      ready; every blocked row still has to name a renderer `view.rs` draws.
- [x] `readiness_is_derived_from_the_verdicts`: the "no pane may go" loop becomes
      "every *blocked* pane row is a blocker", and asserts `info-panel-plugin` is
      not among them.
- [x] `the_build_condition_holds_and_still_gates_a_handover`: its per-pane loop
      skips the handed-over row.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`, and
      the same under `--no-default-features`.

## 8. Acceptance: the pane is the pane

- [x] `src/app/acceptance.rs`: replace every `show_info_panel` use. The tests that
      asserted the native pane's labels (`Agent:`) now assert the seated pane's
      content, and `a_plugin_pane_takes_the_info_panels_seat` loses its
      native-first half — there is no native pane to stand down.
- [x] New: `ToggleInfoPanel` shows the `center-left` pane and hides it again; with
      no pane claiming the seat the column is **not carved** and the action reports
      why; with `[features] info_panel = false` neither happens; below 120 columns
      the seat is not carved with the pane shown.
- [x] New: the empty state — a session-less harness with the pane shown draws a
      bordered `Info` frame, which is the decision this change takes.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and
      `… --no-default-features`.

## 9. Drive it by hand

- [x] `scripts/dev/sandbox.sh --fresh`: at ≥120 columns, F2 shows the Info column
      drawn by the plugin; the gauges tick; F2 hides it; `[features] info_panel =
      false` live-removes it; F2 then reports the switch.
- [x] Break the materialized `init.luau`. **Two cases, not one** — a *load* failure
      (top-level `error()`) removes the pane and F2 reports it; a *render* failure
      keeps the pane as `Info (error)`. The first draft of the docs claimed only the
      second; corrected.
- [x] Drive the `--no-default-features` binary too: F2 must carve nothing and must
      not point at a subcommand that build does not ship.
- [x] Record what was observed in `docs/PHASE4-PANE-READINESS.md` §25.

## 10. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-50 — the first handover, the deleted flag, the
      seed decision, the empty state, the click-target fix.
- [x] `docs/PHASE4-PANE-READINESS.md`: §25, and re-verdict §14's fifth item
      (the empty state) as decided.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the info-panel row is ready; what the
      other six wait on.
- [x] `CLAUDE.md`: the info panel is a plugin; `[features] info_panel` gates a
      plugin pane; `--no-default-features` has no info panel.
- [x] Verify: `rumdl check .`.

## 11. Observed failures, so the tests are known non-vacuous

Each was produced deliberately and reverted.

| Probe | Observed failure |
|---|---|
| `a_click_in_the_info_seat_still_starts_a_text_selection`: set the pane's `accepts_input = true`, so a whole-rect target is recorded again | fails — the click is consumed by `PluginPaneRow` and `text_selection` stays `None`, which is the regression the change prevents |
| the gate, before the probe was tightened | `every_pane_row_names_its_native_renderer` and `recorded_verdicts_match_the_tree` both failed: `view.rs` still *mentioned* `info_panel` via `areas.info_panel`, so the row could not be recorded ready. The needle is now `<module>::` |
| the oracle, run with the native builder gone and the recordings untouched | passes, and `git status tests/snapshots/` is empty — the ten recordings are byte-identical, which is the fact ADR-42 was written to secure |
| a broken bundled plugin, driven by hand | a **load** failure removes the pane and F2 reports it (silent before this change); a **render** failure keeps the pane as `Info (error)` with `failed: …`. The two cases were conflated in the first draft of the docs and are now separated |
| the `--no-default-features` binary, driven by hand | F2 carved nothing and pointed at `thurbox-cli plugin doctor`, a subcommand that build does not ship. `NO_INFO_PANE_HINT` now has two spellings |

## 12. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
