# Tasks

## 1. One composition rule, reachable from `ui`

- [x] `src/ui/mod.rs`: move `format_countdown` here from `app::view` as
  `pub fn format_countdown(remaining_secs: u64) -> String`, with its tests; the
  three `app` call sites divide their milliseconds.
  Verify: `cargo nextest run -E 'test(format_countdown)'`
- [x] `src/ui/automations_panel.rs`: `pub fn row_summary(schedule, action, enabled,
  due_in_secs)` composing `<schedule> · <action> · <when>`.
  `src/app/automation.rs`: `automation_schedule_label` + `automation_due_in_secs`,
  and `format_automation_summary` becomes the adapter that calls `row_summary`, so
  the `Ctrl+P` list modal keeps one rule.
  Verify: `cargo nextest run -E 'test(automation_summary)'`

## 2. The published automations section

- [x] `src/session/pane_context.rs`: `AutomationRowSnapshot`, `AutomationsSnapshot
  { entries, cursor, cursor_visible, focused }`, `MAX_AUTOMATION_ROWS`;
  `PaneContext::automations` is the new section and the old `Vec` becomes
  `upcoming_automations: Vec<UpcomingAutomationSnapshot>`. Tests: a cursor past the
  bound is dropped, and the anchor and the drawn cursor are separate fields.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [x] `src/plugin/kernel_state.rs`: rename `automations_table` →
  `upcoming_automations_table`; add `automations_table` building
  `{ entries, cursor?, cursorVisible, focused }` with 1-based `cursor` and rows
  carrying no rendering. Tests for both readers.
  Verify: `cargo nextest run --features plugins -E 'test(kernel_state)'`
- [x] `src/plugin/capabilities.rs`: insert `automations` beside
  `upcomingAutomations` under `Capability::Automations`; the per-capability gating
  test asserts one grant still implies no other, and that this capability grants
  **both** of its readers and nothing else.
  Verify: `cargo nextest run --features plugins -E 'test(capabilities)'`

## 3. The native pane renders its tree

- [x] `src/ui/automations_panel.rs`: `AutomationPaneEntry` carries the summary's
  *parts* and the row id; new `AutomationRow` (name fitted, summary composed,
  enabled, selected, dimmed, match offsets); `resolve_rows(state, width)` as the one
  width-dependent step; geometry-free `automations_tree(rows, cursor, focused)`
  returning a list that names its anchor; `render_automations_pane` paints it through
  `plugin_pane::render_tree` and derives its hitboxes from the same
  `visible_window`.
  Verify: `cargo nextest run -E 'test(automations_panel)'`
- [x] `src/ui/automations_panel.rs`: retain the pre-port span renderer as a
  `#[cfg(test)]` oracle and assert the tree paints cell-for-cell identically across
  every row appearance **and** at a height that forces a scroll; plus a test that
  the hitboxes cover exactly the rows drawn.
  Verify: `cargo nextest run -E 'test(automations_panel)'`
- [x] `src/app/view.rs`: build entries from the parts and keep the click targets.
  Verify: `cargo nextest run -E 'test(automation)'`

## 4. Publish it from `app`

- [x] `src/app/mod.rs`: `build_automations_snapshot` — every automation in pane
  order, cursor clamped then dropped past the bound, `cursor_visible` from focus or
  a global-search preview, empty with `features.automations` off; rename the
  upcoming list's field at its build site.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [x] `src/app/acceptance.rs`: the published section reflects the automation list,
  its cursor, the focus rule, and is empty with the feature off.
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 5. The bundled plugin, its keys and its cursor

- [x] `src/plugin/bundled/automations/plugin.toml`: `capabilities = ["render",
  "automations", "input", "automations-write"]`, one pane, `default_visible =
  false`, and five `[[keybindings]]` (`next`/`prev`/`toggle`/`run`/`delete`).
- [x] `src/plugin/bundled/automations/init.luau`: the two markers, the
  enabled/disabled colour roles, the selected > dimmed > resting precedence, the
  matched-run emphasis, the summary composition with its own countdown, the
  empty-state line, the anchor on its list — plus its **own cursor**: `nil` until a
  key arrives, moved by `onKey`, declined at either edge, and the row it names is
  what `setAutomationEnabled` / `runAutomation` / `deleteAutomation` address by id.
- [x] `src/plugin/discovery.rs`: add it to `BUNDLED`.
  Verify: `cargo nextest run --features plugins -E 'test(bundled)'`
- [x] `src/plugin/bundled/thurbox.d.luau`: the automation-row types, the second
  reader, and why this section's drawn cursor is a section flag where the task
  section's is a per-row one.
  Verify: `./scripts/dev/lint-luau.sh`

## 6. Prove it renders the same pane, and that its keys act

- [x] `tests/bundled_automations_panel.rs` (new, `#![cfg(feature = "plugins")]`):
  tree equality against `automations_tree` over content variants; **frame**
  equality at a height that scrolls; the summary composed by the plugin equals
  thurbox's `row_summary` for every schedule/action/when shape; the fitted-name
  divergence pinned; `the_plugin_declares_every_power_it_uses` (exactly `render` +
  `automations` + `input` + `automations-write`, hidden by default);
  `the_pane_cannot_be_placed_where_the_native_one_sits` — a manifest naming
  `slot = "left"` is refused, which is the layout finding; the plugin's keys move
  **its own** cursor and leave the published one alone; `Space`/`r`/`d` reach the
  database through the write seam; `r` only marks an automation due; and
  `the_wrap_out_of_the_pane_stays_kernel_owned`.
  Verify: `cargo nextest run --features plugins --test bundled_automations_panel`
- [x] `tests/bundled_manifests.rs`: **no change needed** — it enumerates
  `src/plugin/bundled/` from the source tree, so the new pane joined the
  default-visibility rule the moment its directory existed. Verified rather than
  assumed.
  Verify: `cargo nextest run --test bundled_manifests`
- [x] `tests/tasks_pane_input_gap.rs`: its `input-and-cursor-are-disjoint` row is a
  fact about the **kernel's** cursor, not about panes in general — narrow the prose
  to say so and point at this port. The probe and the verdict are unchanged.
  Verify: `cargo nextest run --test tasks_pane_input_gap`
- [x] `tests/teardown_gate.rs`: unchanged, and still records the automations row
  blocked because `src/app/view.rs` names `automations_panel`.
  Verify: `cargo nextest run --test teardown_gate`

## 7. Docs

- [x] `docs/PHASE4-PANE-READINESS.md` §17: the sixth port — what sufficed, the
  capability that grew a reader instead of multiplying, the summary-parts decision
  and the sharpened rule, the anchor/appearance confirmation, the formatter case
  made twice, the left-column finding with its cost table, the keys that ported and
  the two that did not, the wrap's ownership, and the pane-focus gap.
- [x] `docs/ARCHITECTURE.md`: ADR-41 for the second reader, the published parts, the
  separate anchor, and the plugin's own cursor.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the pane table's automations row now
  names its plugin (still drawn natively).
- [x] `CLAUDE.md`: the second automations reader and the bundled pane with keys.
  Verify: `rumdl check .`

## 8. Unplanned, found while implementing

- [x] `src/session/keybindings.rs`: the chord grammar could not spell the **space
  bar** — `display` emitted a literal `" "` that `parse` trims to nothing, so the
  default chord of `AutomationsToggle`/`TasksCycleStatus` could not round-trip
  through `keybindings.json` and no manifest could declare it. Named in both
  directions, with a regression test. Required by this plugin's toggle binding.
  Verify: `cargo nextest run -E 'test(chord) or test(space_bar)'`
- [x] `src/ui/automations_panel.rs`: the pane had **two rules for one cursor** —
  it clamped the index it windowed on and compared the unclamped one to pick the
  highlighted row. The host refuses a list whose cursor is not an index into its
  children, so that state is not expressible by a pane; the appearance now follows
  the anchor. One cell-level behaviour change, pinned against the retained oracle
  by `a_stale_selection_now_highlights_the_last_row_rather_than_none`.
- [x] `src/ui/highlight.rs`: `row_base_style` lost its last production caller when
  this pane started naming style *tokens*, so it is `#[cfg(test)]` — it is what
  both retained oracles resolve a row's base through.

## 9. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (≥ 2685, 0 failed)
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
  (≥ 2685, 0 failed)
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `./scripts/dev/lint-luau.sh` ; `./scripts/dev/lint-workflows.sh` ;
  `rumdl check .` ; no `.snap.new`
- [x] Driven by hand: `scripts/dev/sandbox.sh --fresh --show automations`, then
  `tmux send-keys` for `Ctrl+L` onto the pane, `j`, `Space`, `r`, `Esc`.
