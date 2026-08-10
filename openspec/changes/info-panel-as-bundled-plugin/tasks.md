# Tasks

## 1. Make `info_tree` a pure function of its inputs

- [x] `src/ui/info_panel.rs`: add `now: u64` to `info_tree` and
  `push_usage_rows`; `render_info_panel` resolves it with `epoch_now_secs()`.
  Update the test module's call sites.
  Verify: `cargo nextest run -E 'test(ui::info_panel)'` and the pinned frame does
  not move (`src/ui/snapshots/` holds no `.snap.new`)

## 2. The published snapshot

- [x] `src/session/pane_context.rs` (new) + `src/session/mod.rs`: `PaneContext`
  with its `SessionSnapshot` / `SystemSnapshot` / `AutomationSnapshot` sections,
  the nested `StatusSnapshot` / `GitSnapshot` / `AgentMetricsSnapshot` /
  `UsageSnapshot` / `UsageWindowSnapshot`, `publish()` / `published()` over a
  process-wide `RwLock`, and `readers_present()` / `set_readers_present()` over an
  `AtomicBool`. References only `std` and `super`.
  Verify: `cargo nextest run -E 'test(session::pane_context)'`
- [x] `src/session/pane_context.rs`: `StatusSnapshot::of(SessionStatus)` carrying
  label, glyph and the `StyleToken` wire name, so the kernel's status→token
  mapping is not re-derived in Luau.
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 3. Publish it from `app`

- [x] `src/app/metrics_state.rs`: add the `pane_context_builds` /
  `pane_context_publishes` perf counters.
  Verify: `cargo nextest run -E 'test(perf_)'`
- [x] `src/app/mod.rs`: `App::build_pane_context` (from the same inputs
  `render_info_panel` uses — active session, parent name, system metrics, usage,
  the filtered automation cache, `thurbox_dir_bytes`) and
  `App::publish_pane_context`, gated on `readers_present()` then on inequality
  with the last published value; called at the end of `tick_core`. It must not
  mark the UI dirty.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [x] `src/app/acceptance.rs`: assert no snapshot is built with no readers, that
  an unchanged tick publishes at most once, that a changed value publishes, and
  that publishing does not mark dirty.
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 4. The capabilities and their readers

- [x] `src/session/plugin_manifest.rs`: `Capability::Sessions` / `Metrics` /
  `Automations` with wire names `sessions` / `metrics` / `automations`, in
  `as_str` and `all`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_manifest)'`
- [x] `src/plugin/kernel_state.rs` (new) + `src/plugin/mod.rs`: convert each
  `PaneContext` section to a Lua table.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::kernel_state)'`
- [x] `src/plugin/capabilities.rs`: insert `activeSession` / `systemMetrics` /
  `upcomingAutomations` under their capabilities, each reading
  `pane_context::published()` at call time; test presence, absence, and that one
  grant does not imply another.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`
- [x] `src/plugin/lifecycle.rs`: `publish_state_demand`, called from every entry
  point that changes what is running (`start_all`, `reload`, `reset`,
  `stop_all`), derived from the grants of running plugins.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::lifecycle)'`

## 5. The bundled plugin

- [x] `src/plugin/bundled/info-panel/plugin.toml`: `capabilities = ["render",
  "sessions", "metrics", "automations"]`, one pane with
  `default_visible = false`.
- [x] `src/plugin/bundled/info-panel/init.luau`: the pane, reimplementing every
  formatter and every row of `info_tree` in Luau.
- [x] `src/plugin/discovery.rs`: add it to `BUNDLED`.
  Verify: `cargo nextest run --features plugins -E 'test(the_bundled_plugin_is_valid)'`
- [x] `src/plugin/bundled/thurbox.d.luau`: declare the three readers and their
  snapshot types.
  Verify: `./scripts/dev/lint-luau.sh`

## 6. Prove it renders the same pane

- [x] `tests/bundled_info_panel.rs` (new, `#![cfg(feature = "plugins")]`): run
  the bundled plugin through a real `PluginHost` over
  `src/plugin/bundled/info-panel/` and assert its tree equals `info_tree`'s for a
  fully populated fixture and for each optional section omitted; assert the pane
  is hidden by default and that the plugin declares no undeclared reach.
  Verify: `cargo nextest run --features plugins --test bundled_info_panel`

## 7. Tighten the teardown gate

- [x] `tests/teardown_gate.rs`: `pane()` takes the native renderer's module name
  and its probe requires both the bundled plugin and the absence of that name from
  `src/app/view.rs`; add a test asserting the info-panel plugin exists while its
  row stays blocked.
  Verify: `cargo nextest run --test teardown_gate`

## 8. Docs

- [x] `docs/PHASE4-PANE-READINESS.md`: §2 closed, with the shape it took and the
  two costs it did not pay off (freshness, formatter duplication).
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: record the stricter pane verdict.
- [x] `docs/ARCHITECTURE.md`: an ADR for the published-snapshot state channel.
- [x] `docs/PERFORMANCE.md`: the two counters and the two gates.
- [x] `CLAUDE.md`: the three capabilities, the bundled pane, and the snapshot's
  division of labour.
  Verify: `rumdl check .`

## 9. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `cargo nextest run --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh` ; `rumdl check .`
