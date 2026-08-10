# Tasks

## Part A — the keyboard reaches every pane

- [x] 1. Add `PluginPaneRow` + `PluginPanesModal` and the `Modal::PluginPanes`
  variant, with its `list_selection` arm returning `KeyCode::Char(' ')`.
  Files: `src/app/modals.rs`.
  Verify: `cargo check --features plugins && cargo check`
- [x] 2. Split the toggle: `App::set_plugin_pane_visible(plugin, pane, visible)`
  as the single write path (flip + persist, keyed by name so a reload cannot
  redirect it), `App::toggle_plugin_pane` dispatching on
  the declared-pane count (0 → nothing, 1 → toggle, ≥2 → open the picker), and
  `App::open_plugin_panes_picker` building the rows from `self.plugin_panes`.
  Files: `src/app/mod.rs`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_pane)'`
- [x] 3. Route the modal's keys (`j`/`k`/arrows, `Space`, `Enter`, `Esc`) and add
  the `modal_opener_pressed` arm so the bound action closes it again.
  Files: `src/app/key_handlers.rs`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_pane)'`
- [x] 4. Render it: a checkbox per row, plugin-qualified label, selector footer.
  Files: `src/ui/plugin_panes_modal.rs` (new), `src/ui/mod.rs` (module
  declaration), `src/app/view.rs` (one arm in `render_selector_modal`).
  Verify: `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] 5. Acceptance tests over the harness: one pane toggles directly with no
  modal; two panes open the picker and the *second* pane can be shown; the
  picker's toggle stores the same value the generated command stores; `Esc`
  changes nothing further; zero panes is a silent no-op; a row addresses its pane
  by name so a reload cannot make it toggle the wrong one; and **every bundled
  pane** answers the key, driven over the real `src/plugin/bundled/` set.
  Files: `src/app/acceptance.rs`.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins -E 'test(plugin_pane)'`

## Part B — a hidden pane is not rendered

- [x] 6. Add the published slot: `HiddenPane`, `publish_hidden`, `is_hidden`,
  plus the test lock/clear helpers the crate's other published slots expose.
  Files: `src/session/pane_visibility.rs` (new), `src/session/mod.rs`.
  Verify: `cargo nextest run -E 'test(pane_visibility)'`
- [x] 7. Skip hidden panes in the host and count VM renders (`render_calls`).
  Files: `src/plugin/lifecycle.rs`.
  Verify: `cargo nextest run --features plugins -E 'test(hidden_pane)'`
- [x] 8. Publish from the tick behind a change gate, with the
  `pane_visibility_publishes` counter, next to `publish_pane_context`.
  Files: `src/app/mod.rs`, `src/app/metrics_state.rs`.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins -E 'test(perf_) + test(pane_visibility)'`
- [x] 9. Perf test: publishing is change-gated (a run of ticks with nothing
  moving advances no counter) and a hidden pane costs one fewer VM render.
  Files: `src/app/acceptance.rs`, `src/plugin/lifecycle.rs`.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`

## Docs and gates

- [x] 10. `CLAUDE.md`: the `F10` keybinding row and the plugin-pane paragraph.
  `docs/ARCHITECTURE.md`: ADR-28 (the picker + the published hidden set).
  `docs/PHASE4-PANE-READINESS.md`: §5 closed, with what the port of the *next*
  pane can now assume. `docs/PERFORMANCE.md`: the new counter.
  Verify: `rumdl check .`
- [x] 11. Confirm no new module edge and no teardown row flips.
  Verify: `cargo nextest run --test architecture_rules --test teardown_gate` and
  `git diff --stat tests/architecture_rules.rs tests/teardown_gate.rs` (expected
  empty)
- [x] 12. Full verification before commit: `cargo fmt --all -- --check`;
  `cargo clippy --all-targets --features plugins -- -D warnings`;
  `cargo clippy --all-targets -- -D warnings`;
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`;
  `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`;
  `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`;
  `cargo tree --edges normal | grep -c mlua` (must be 0);
  `./scripts/dev/lint-luau.sh`; `rumdl check .`
