# Tasks

## 1. One composition rule, reachable from `ui`

- [ ] `src/ui/mod.rs`: move `format_countdown` here from `app::view` as
  `pub fn format_countdown(remaining_secs: u64) -> String`, with its tests; the
  three `app` call sites divide their milliseconds.
  Verify: `cargo nextest run -E 'test(format_countdown)'`
- [ ] `src/ui/automations_panel.rs`: `pub fn row_summary(schedule, action, enabled,
  due_in_secs)` composing `<schedule> · <action> · <when>`.
  `src/app/automation.rs`: `automation_schedule_label` + `automation_due_in_secs`,
  and `format_automation_summary` becomes the adapter that calls `row_summary`, so
  the `Ctrl+P` list modal keeps one rule.
  Verify: `cargo nextest run -E 'test(automation_summary)'`

## 2. The published automations section

- [ ] `src/session/pane_context.rs`: `AutomationRowSnapshot`, `AutomationsSnapshot
  { entries, cursor, cursor_visible, focused }`, `MAX_AUTOMATION_ROWS`;
  `PaneContext::automations` is the new section and the old `Vec` becomes
  `upcoming_automations: Vec<UpcomingAutomationSnapshot>`. Tests: structural
  equality (the change gate needs it), a cursor past the bound is dropped, and the
  anchor and the drawn cursor are separate fields.
  Verify: `cargo nextest run -E 'test(session::pane_context)'`
- [ ] `src/plugin/kernel_state.rs`: rename `automations_table` →
  `upcoming_automations_table`; add `automations_table` building
  `{ entries, cursor?, cursorVisible, focused }` with 1-based `cursor` and rows
  carrying no rendering. Tests for both readers.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::kernel_state)'`
- [ ] `src/plugin/capabilities.rs`: insert `automations` beside
  `upcomingAutomations` under `Capability::Automations`; the per-capability gating
  test asserts one grant still implies no other, and that this capability grants
  **both** of its readers and nothing else.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`
- [ ] `src/session/plugin_manifest.rs`: extend `Capability::Automations`'s doc to
  name both readers and why they share one capability.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_manifest)'`

## 3. The native pane renders its tree

- [ ] `src/ui/automations_panel.rs`: `AutomationPaneEntry` carries the summary's
  *parts*; new `AutomationRow` (name fitted, summary composed, enabled, selected,
  dimmed, match offsets); `resolve_rows(state, width)` as the one width-dependent
  step; geometry-free `automations_tree(rows, cursor, focused)` returning a list
  that names its anchor; `render_automations_pane` paints it through
  `plugin_pane::render_tree` and derives its hitboxes from the same
  `visible_window`.
  Verify: `cargo nextest run -E 'test(ui::automations_panel)'`
- [ ] `src/ui/automations_panel.rs`: retain the pre-port span renderer as a
  `#[cfg(test)]` oracle and assert the tree paints cell-for-cell identically across
  every row appearance **and** at a height that forces a scroll; plus a test that
  the hitboxes cover exactly the rows drawn.
  Verify: `cargo nextest run -E 'test(ui::automations_panel)'`
- [ ] `src/app/view.rs`: build entries from the parts and keep the click targets.
  Verify: `cargo nextest run -E 'test(automation)'`

## 4. Publish it from `app`

- [ ] `src/app/mod.rs`: `build_automations_snapshot` — every automation in pane
  order, cursor clamped then dropped past the bound, `cursor_visible` from focus or
  a global-search preview, empty with `features.automations` off; rename the
  upcoming list's field at its build site.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [ ] `src/app/acceptance.rs`: the published section reflects the automation list,
  its cursor, the focus rule, is empty with the feature off, and publishing stays
  change-gated on an idle tick.
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 5. The bundled plugin

- [ ] `src/plugin/bundled/automations/plugin.toml`: `capabilities = ["render",
  "automations"]`, one pane, `default_visible = false`.
- [ ] `src/plugin/bundled/automations/init.luau`: the two markers, the
  enabled/disabled colour roles, the selected > dimmed > resting precedence, the
  matched-run emphasis, the summary composition with its own countdown, the
  empty-state line, and the anchor on its list.
- [ ] `src/plugin/discovery.rs`: add it to `BUNDLED`.
  Verify: `cargo nextest run --features plugins -E 'test(bundled)'`
- [ ] `src/plugin/bundled/thurbox.d.luau`: the automation-row types and the second
  reader.
  Verify: `./scripts/dev/lint-luau.sh`

## 6. Prove it renders the same pane

- [ ] `tests/bundled_automations_panel.rs` (new, `#![cfg(feature = "plugins")]`):
  tree equality against `automations_tree` over content variants; **frame**
  equality at a height that scrolls; the summary composed by the plugin equals
  thurbox's `row_summary` for every schedule/action/when shape; the fitted-name
  divergence pinned; `the_plugin_declares_every_power_it_uses` (exactly `render` +
  `automations`, hidden by default); and
  `the_pane_cannot_be_placed_where_the_native_one_sits` — a manifest naming
  `slot = "left"` is refused, which is the layout finding.
  Verify: `cargo nextest run --features plugins --test bundled_automations_panel`
- [ ] `tests/teardown_gate.rs`: unchanged, and still records the automations row
  blocked because `src/app/view.rs` names `automations_panel`.
  Verify: `cargo nextest run --test teardown_gate`

## 7. Docs

- [ ] `docs/PHASE4-PANE-READINESS.md` §10: the fourth port — what sufficed, the
  capability that grew a reader instead of multiplying, the summary-parts decision
  and the sharpened rule, the anchor/appearance confirmation, the formatter case
  made twice, and the left-column finding with its cost table.
- [ ] `docs/ARCHITECTURE.md`: ADR-31 for the second reader, the published parts, and
  the separate anchor.
- [ ] `docs/PHASE6-TEARDOWN-READINESS.md`: the pane table's automations row now
  names its plugin (still drawn natively), and the stale capability list is
  corrected.
- [ ] `CLAUDE.md`: the second automations reader and the bundled pane.
  Verify: `rumdl check .`

## 8. Full verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --features plugins -- -D warnings`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [ ] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (≥ 2184, 0 failed)
- [ ] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
  (≥ 2547, 0 failed)
- [ ] `cargo tree --edges normal | grep -c mlua` → 0
- [ ] `./scripts/dev/lint-luau.sh` ; `rumdl check .` ; no `.snap.new`
