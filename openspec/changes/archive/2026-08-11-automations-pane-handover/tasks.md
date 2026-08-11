# Tasks

## 1. Relocate the one thing the module owns twice

- [x] `src/ui/automations_list_modal.rs`: declare `row_summary` here, with a doc saying
      why it is not in `ui/mod.rs` (that is the layer's shared vocabulary; this is one
      surface's row format) and not in `app` (it is display-text composition, and its
      countdown helper lives in `ui`).
- [x] `src/app/automation.rs`: `format_automation_summary` calls the new path.
- [x] Verify: `cargo check --all`.

## 2. The bundled pane takes the pane's identity, and gives up its keys

- [x] `src/plugin/bundled/automations/plugin.toml`: `title = "Automations"`,
      `feature = "automations"`, `key_context = "Automations"`,
      `default_visible = true`; `capabilities = ["render", "automations"]`; the five
      `[[keybindings]]` deleted; no `toggle_action`, because the native band had none.
- [x] `src/plugin/bundled/automations/init.luau`: delete the pane's own cursor, `move`,
      `selectedEntry`, `onKey` and `onClick`; draw the host's `cursor` /
      `cursorVisible`. Rewrite the header — it describes a reproduction that drives
      itself, and this file now only draws.
- [x] `tests/bundled_manifests.rs`: `PANES_DRAWN_IN_A_NATIVE_PANES_PLACE` carries the
      native default per entry; the rule becomes "seeds at the native pane's default",
      and a toggle action is required only of a pane that seeds hidden.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_manifests)'`, `./scripts/dev/lint-luau.sh`.

## 3. Stop drawing it, and delete the kernel's occupant of the seat

- [x] `src/app/view.rs`: delete `App::render_automations_pane` and its call; drop
      `automations_panel` from the `use crate::ui::{…}` list.
- [x] `src/app/mod.rs`: `layout_for`'s `show_automations_pane` becomes
      `self.seat_taken(PaneSlot::LeftBottom)`; `lower_left_rows` loses its native
      fallback and answers 0 without the host.
- [x] `src/app/key_handlers.rs`: `act_session_list_next`/`act_session_list_prev` gate
      the wrap on `pane_keyboard_taken(KeyContext::Automations)` rather than on
      `features.automations`.
- [x] `src/app/search.rs`: the automation jump focuses the pane only when one provides
      the list, and reports which plugin does otherwise.
- [x] `src/ui/automations_panel.rs`: **deleted**; `src/ui/mod.rs` drops the module.
- [x] Keep the `[features] automations` focus rescue in `apply_live_settings` — the
      flag was answering "is the pane on screen" for focus, and that question survives
      it.
- [x] Verify: `cargo check --all && cargo check --all --no-default-features`.

## 4. The oracle keeps its recordings, one rule, and loses the builder

- [x] `tests/bundled_automations_panel.rs`: drop the `automations_tree` / `resolve_rows`
      edges and the native-rows plumbing; `Case` builds the published section directly;
      the recordings are the expectation. Delete the five key tests and the writer
      plumbing with the keys they measured. Keep
      `the_plugin_composes_the_summary_thurbox_composes` against
      `ui::automations_list_modal::row_summary`, and say in the module note why that one
      edge is not differential.
- [x] Verify the recordings did **not** move: `git status tests/snapshots/` is empty
      after the deletion.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_automations_panel)'`.

## 5. The gates

- [x] `tests/teardown_gate.rs`: `automations-plugin` becomes `ready`; the handed-over
      list gains it; `EXAMPLE_BLOCKED_PANE` moves to a still-native pane.
- [x] `tests/automations_pane_handover_gap.rs`: **deleted**, with its rows preserved in
      ADR-56.
- [x] `tests/global_search_pane_gap.rs`: its `no-cross-pane-styling` row reads this
      pane's *plugin* now, not its module — one native pane left of the three the row
      names. (`tests/session_list_pane_handover_gap.rs` needed nothing: no row of it
      cites the native automations pane.)
- [x] `src/app/acceptance.rs`: a `seat_automations_pane` helper mirroring
      `seat_tasks_pane`; the monkey invariant for the three automation focuses becomes
      "a pane provides the automations list"; every test that drove the native band
      seats the pane first.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` and
      `--test architecture_rules`.

## 6. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-56, carrying the retired gate's table and the
      retired port's key claims.
- [x] `docs/PHASE4-PANE-READINESS.md` §31.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the third `ready` row.
- [x] `CLAUDE.md`: the automations-pane section.
- [x] Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `rumdl check .`.

## 7. Full verification, and driving it

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] `openspec validate automations-pane-handover --strict`
- [x] Hand-drive: the band is in the native position with the native frame; `j` from
      the last session drops into it and `k` returns; `j`/`k` move the cursor;
      `Space` toggles (confirmed via `automation list`); `r` runs; `d` deletes;
      `n` and `Enter` open the central editor and its run history; `Esc` leaves;
      `[features] automations = false` removes the band live; the
      `--no-default-features` binary carves nothing and `Ctrl+P` still authors.
