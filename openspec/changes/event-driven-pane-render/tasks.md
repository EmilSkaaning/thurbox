# Tasks

## 1. Name what a pane reads

- [x] `src/session/plugin_manifest.rs`: add `PaneSource` (7 members) and
      `SourceSet` (a `u8` bitset with `empty`/`of`/`insert`/`contains`/`is_empty`/
      `union`/`intersects`), and `Capability::source() -> Option<PaneSource>` as an
      exhaustive match with no wildcard arm.
- [x] Tests: every capability's `source()` agrees with `reads_kernel_state()` —
      `Some` and not `plugin-state` exactly for the six readers, `Some(plugin-state)`
      exactly for `state-read`, `None` otherwise; and `SourceSet`'s set algebra.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E
      'test(plugin_manifest)'`.

## 2. Say what moved

- [x] `src/session/pane_context.rs`: `PaneContext::changed_sources(&self, other)
      -> SourceSet`, destructuring **both** snapshots by name with no `..`, so a new
      field cannot belong to no source. Document that it never returns
      `plugin-state`.
- [x] Test the equivalence with `==` over a table of one-field mutations, one per
      field of the snapshot, so the change gate and the nudge cannot disagree.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(pane_context)'`.

## 3. The trigger, pure

- [x] `src/plugin/render_trigger.rs`: `PaneRef`, `PaneReads { pane, sources }`,
      `RenderTrigger` with `state_moved(SourceSet)`, `everything_moved()`,
      `pane_took_input(plugin, pane)`, `plugin_state_may_have_moved()`,
      `due(now) -> Due::{Now, Throttled(Duration), Idle}`,
      `wanted(&[PaneReads]) -> Vec<PaneRef>`, and `settle(now, rendered_any)`.
      `settle` advances the rate clock only when a pane was actually rendered, so a
      change no visible pane reads does not delay the next real render.
- [x] `src/plugin/mod.rs`: declare and re-export it.
- [x] Unit tests, clock passed in so nothing sleeps: idle stays idle; a moved source
      is due immediately; a second change inside the interval is `Throttled` with the
      remainder; `wanted` selects only panes whose sources intersect; a forced pane is
      selected whatever moved; `everything_moved` selects every pane; a source nobody
      reads yields an empty `wanted` and leaves the rate clock alone.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E
      'test(render_trigger)'`.

## 4. What the host can answer

- [x] `src/plugin/lifecycle.rs`: `PluginHost::pane_reads() -> Vec<PaneReads>` (visible
      panes only, sources unioned from each slot's **granted** set, mirroring
      `pane_bindings`' reason for reading the grant rather than the request), and
      `render_pane_collected(plugin, pane)` for one pane. Replace
      `render_all_panes_collected`'s only caller and keep or retire it deliberately.
- [x] Tests: a pane's sources come from the grant, a hidden pane is absent, a pane of
      a failed plugin is absent.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(lifecycle)'`.

## 5. The nudge

- [x] `src/app/mod.rs`: `PluginWorkerRequest::{Input, StateMoved(SourceSet),
      RenderAll}`; rename `plugin_keys` to `plugin_worker` and change its type;
      `set_plugin_channels` follows.
- [x] `publish_pane_context`: gate on `changed_sources` being non-empty and send
      `StateMoved` with it. `publish_plugin_pane_visibility`: send `RenderAll` on a
      change. Neither may mark the interface dirty.
- [x] `offer_key_to_plugin` / the click path: wrap in `PluginWorkerRequest::Input`.
- [x] `src/app/metrics_state.rs`: `plugin_renders_applied` and
      `plugin_renders_changed`, with their window deltas and the perf JSON.
- [x] `poll_plugin_renders`: bump both.

## 6. The loop

- [x] `src/main.rs`: drive the trigger — source-file poll on its own cadence
      (`PLUGIN_SOURCE_POLL`), render only what `wanted` names, serve input while
      waiting, cap the wait at one slice so shutdown is as prompt as before. Retire
      `PLUGIN_RENDER_SLICES` and replace `PLUGIN_RENDER_SLICE`'s render role with
      `PLUGIN_RENDER_MIN_INTERVAL`.
- [x] Keep the first pass rendering every pane, so a pane exists as soon as the host
      arrives.

## 7. Tests that pin the behaviour end to end

- [x] `src/app/acceptance.rs`: a publication nudges with the sources that moved and
      not the others; an unchanged snapshot nudges nothing; a visibility change asks
      for every pane; a nudge does not mark the interface dirty.
- [x] `src/app/acceptance.rs`: identical trees cost no repaint, asserted on
      `plugin_renders_applied` / `plugin_renders_changed` and `should_redraw()` —
      the non-negotiable property, on counters rather than on wall-clock timing.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and the same with
      `--no-default-features`.

## 8. Drive it by hand

Driven in a sandbox with two bundled panes shown and no agent involved. Observed:

| Check | Result |
|---|---|
| the info panel's gauges move | CPU 28% → 30% → 29% → 29% across four 1.3 s samples |
| a key's effect reaches the plugin's copy | `Space` on a task changed the glyph in **both** the native pane and the plugin's copy within 120 ms |
| an external write reaches it | `thurbox-cli task edit --status done` from another process landed on the next task refresh, costing **1** render out of that window's 20 publications |
| idle costs nothing | 20 s / 2000 ticks with one visible pane reading `tasks`: 20 publications, **0** renders, 0 repaints |
| idle with a metrics pane | 20 s: 20 publications, 28 renders (the info panel reads `metrics`, which moves ~1 Hz) |

- [x] `scripts/dev/sandbox.sh --fresh`, show the bundled info-panel pane, and confirm
      its gauges and countdowns move without a perceptible lag; move the session-list
      cursor with the session-list pane shown and confirm its copy follows in the same
      breath rather than a second later.
- [x] With `THURBOX_PERF_LOG=1`, confirm an idle TUI with a visible plugin pane
      reports no render churn.

## 9. Documentation

- [x] `docs/PHASE4-PANE-READINESS.md`: a section closing §14's last row — the
      measured publish rate that refutes the "~100 Hz" objection, the ceiling and its
      worst case, the one surviving timer, and what the bundled set's idle cost is now.
- [x] `docs/PERFORMANCE.md`: the trigger and the two new counters.
- [x] `docs/ARCHITECTURE.md`: an ADR for the event-driven trigger and the rate policy.
- [x] `CLAUDE.md`: the plugin-pane paragraph's claim about when a pane renders.

## 10. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — every pane row
      still blocked; this change hands nothing over
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
- [x] `openspec validate event-driven-pane-render --strict`
