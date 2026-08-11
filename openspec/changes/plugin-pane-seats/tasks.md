# Tasks

## 1. The slot vocabulary and its seat table

- [x] `src/session/plugin_manifest.rs`: `PaneSlot` gains `Left`, `LeftBottom`,
      `CenterLeft`, `Center`; `as_str`, `all`, `Display`, and
      `seat() -> Option<RegionId>` mapping each to the region the workspace tree
      places (`Right` → `None`).
- [x] Unit tests: every slot parses from its wire name, an unknown slot is a
      manifest error naming it, an omitted slot is `Right`, each seat maps to a
      distinct region, and no seat names header/footer/search/status.
- [x] Verify: `cargo nextest run -E 'test(plugin_manifest)'`.

## 2. The one content-derived height

- [x] `src/session/view_tree.rs`: `ViewNode::stacked_row_count()` — the outermost
      `List`/`Column`'s child count, 1 otherwise, documented as a row count and not
      a rendered height.
- [x] Unit tests: a list, a column, a bare text node, an empty list.
- [x] Verify: `cargo nextest run -E 'test(stacked_row_count)'`.

## 3. Seating in the app

- [x] `src/app/mod.rs`: `plugin_seat(slot)` (first visible pane declaring it),
      `seat_taken(slot)` (+ a `#[cfg(not(feature = "plugins"))]` `false`),
      `visible_plugin_panes()` counts only right-column panes, and `layout_for`
      ORs each claim into the flag that carves the seat and feeds the lower-left
      band the seated pane's row count.
- [x] `src/app/view.rs`: `render_left_panel` / `render_automations_pane` /
      `render_info_panel` / `render_central_pane` return early when their seat is
      taken; `render_plugin_panes` takes the whole `PanelAreas`, paints each seated
      pane into its seat's rect and the rest into the right column, and records the
      same click targets for both.
- [x] `src/ui/layout.rs`: doc only — the automations seat's occupant is no longer
      fixed and its count is a content-row count.
- [x] Verify: `cargo nextest run -E 'test(layout)'`.

## 4. Acceptance: the seat is real, and the geometry is not new

- [x] `src/app/acceptance.rs`: a `center-left` pane is drawn in the info column and
      the native info panel is not; the seated pane's rect equals the native
      pane's; a claim carves a seat the user toggled off; a `left-bottom` pane's
      band height follows its stacked row count; a `center` pane replaces the
      central view and hiding it restores it (with the tab strip back); two panes
      claiming one seat draw once; no claim leaves every rect unchanged.
- [x] Verify: `cargo nextest run -E 'test(seat)'`,
      `cargo nextest run -E 'test(acceptance)'`, snapshots unchanged.

## 5. The three reproductions move into their seats

- [x] `src/plugin/bundled/session-list/plugin.toml` → `slot = "left"`,
      `automations/plugin.toml` → `left-bottom`, `info-panel/plugin.toml` →
      `center-left`; each still `default_visible = false`.
      (The Luau declaration file describes the *API* a plugin calls, not its
      manifest, so it carries no slot vocabulary to update.)
- [x] `tests/bundled_automations_panel.rs`: the placement divergence is retired —
      the pane now names the seat its native counterpart occupies.
- [x] Verify: `cargo nextest run -E 'test(bundled)'`,
      `./scripts/dev/lint-luau.sh`.

## 6. Re-verdict the six gate rows

- [x] `tests/automations_pane_handover_gap.rs`, `tests/session_list_pane_handover_gap.rs`:
      `no-left-seat` closes, with the seat and the height policy as its probe.
- [x] `tests/tasks_pane_input_gap.rs`: `no-central-seat` closes as a seat row.
- [x] `tests/code_review_pane_handover_gap.rs`: `no-central-seat` closes;
      `no-second-seat-for-the-changed-files-list` stays blocked on a probe reading
      that no slot names `RegionId::FileViewer`; the two-panes assertion is
      rewritten.
- [x] `tests/global_search_pane_gap.rs`: `no-band-slot` stays blocked on a probe
      reading that no slot names `RegionId::GlobalSearch`.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`.

## 7. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-46 — a slot names a region, the plugin wins the
      seat, a claim carves it, the content-derived height, and what the centre does
      not get.
- [x] `docs/PHASE4-PANE-READINESS.md`: §14's "the same seat" row closes; a new
      section records what the seat closed, what it did not (chrome, focus), and
      the six re-verdicted rows.
- [x] `CLAUDE.md`: the plugin-host paragraph's slot sentence.
- [x] Verify: `rumdl check .`.

## 8. Whole-tree verification

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
- [x] By hand: `scripts/dev/sandbox.sh --fresh`, show the `info-panel` pane with
      `F10` and confirm it is drawn in the info column with the native pane gone,
      then hide it and confirm the native pane returns.
