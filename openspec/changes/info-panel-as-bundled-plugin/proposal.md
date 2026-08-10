# Render the info panel from a bundled Luau plugin

## Why

Phase 4 of the v2 migration turns thurbox's native panes into bundled plugins,
easiest first, and `docs/v2/MIGRATION.md` puts the info panel first. Phase 0
already proved half of it: `ui::info_panel::info_tree` builds a
`session::view_tree::ViewNode` and the shared renderer paints it, so the
**catalogue** can express the pane. What it could not prove is that a *plugin*
could produce that tree, and `docs/PHASE4-PANE-READINESS.md` §2 says why in one
sentence: **no host binding reads kernel state.** `plugin::capabilities::build_module_table`
grants `name`, `log`, the `state*` trio over the plugin's own namespace, and the
`ui` constructors — and nothing else. A pane that renders a session cannot be
written at all today, not badly, not at all.

So the v1 behaviour being replaced is not a rendering path — Phase 0 already
moved that — it is **ownership**. Today `src/app/view.rs` reads the active
session out of `App`, resolves its parent name, filters the automation cache,
and hands all of it to a Rust function on the UI thread. After this change a
Luau plugin in its own VM, on the plugin render worker, reads the same values
through three capability-gated bindings and returns the same tree.

The native pane stays compiled in and stays the one on screen by default.
Deleting it is Phase 6, and `tests/teardown_gate.rs` guards that — this change
makes that gate *stricter*, because its probe for a pane's replacement asks only
whether a bundled plugin directory exists, and a plugin drawing a pane
*alongside* the native one has not replaced it.

## What Changes

- **A published kernel-state snapshot.** New `session::pane_context`: pure data
  plus a process-wide `RwLock<Option<PaneContext>>`, on the precedent
  `session::spawn_contribution` set. `app` builds and publishes it; `plugin`
  reads it when a binding is called. No plugin code runs on the UI thread and no
  new architecture edge appears.
- **Three new capabilities**, `sessions` / `metrics` / `automations`, each
  gating one reader: `thurbox.activeSession()`, `thurbox.systemMetrics()`,
  `thurbox.upcomingAutomations()`. Three rather than one because an install
  prompt saying "reads your sessions" must not silently also read host CPU.
- **The snapshot publishes what a plugin cannot derive, and nothing else.** The
  sandbox has no `os` and no path library, so the kernel resolves the clock
  (`resets_in_secs`, not `resets_at`), path basenames, the parent session's
  name, and a status's glyph and style token. Quantities stay raw numbers and
  every string the pane displays is composed in Luau — otherwise the plugin
  would be arranging strings the kernel formatted.
- **Publishing is demand-gated and change-gated.** Nothing is built unless a
  running plugin holds one of the three capabilities
  (`pane_context::readers_present()`, an `AtomicBool` the host sets), and
  nothing is published unless the value differs from what was last published.
  Two perf counters assert both, so the guarantee is not prose.
- **`info_tree` loses its clock.** It called `epoch_now_secs()` to build the
  usage countdown; `now` becomes a parameter, so the tree is a pure function of
  its inputs and a differential test against the plugin is exact rather than
  minute-boundary flaky.
- **A bundled `info-panel` plugin**, shipped inside the binary next to `hello`,
  `default_visible = false` so no user's layout changes. It reimplements every
  one of the pane's formatters in Luau and is asserted to produce a view tree
  **equal** to `info_tree`'s across content variants — which is byte-identity of
  the painted pane, since the same renderer paints the same tree.
- **`tests/teardown_gate.rs`**: a pane's replacement is ready only when the
  plugin exists **and** `src/app/view.rs` no longer names the native renderer.

## Capabilities

- `plugin-host/kernel-state` — ADDED: the published snapshot, its three
  capability-gated readers, what the kernel resolves on the plugin's behalf, and
  the demand/change gates on publishing.
- `plugin-host/capabilities` — ADDED: reading kernel state is a declared
  capability, per kind of state.
- `migration/phase-4` — ADDED: what a pane's port to a bundled plugin must
  demonstrate, and that the native pane survives it.
- `migration/teardown` — ADDED: a pane's replacement verdict means handover, not
  coexistence.

## Non-goals

- **Deleting or unwiring the native info panel.** It is what every user sees;
  the plugin is additive and hidden by default. Handover is Phase 6.
- **Making the plugin pane update at frame rate.** The render worker polls on a
  ~1 s cycle, so the plugin's copy of a live gauge lags the native pane by up to
  a second. `docs/SPIKE-SESSION-LIST.md` already fixed event-driven render as a
  condition of the session-list port; this change measures the consequence and
  leaves the condition open rather than pre-empting it.
- **A `sessions()` reader for the whole list.** The `sessions` capability is
  named for the reach, not for the one function that exists; the list reader
  lands with the session-list port that needs it.
- **Per-pane keyboard visibility** (PHASE4 §5). The info panel takes no keys.
- **Widening the view tree.** Phase 0 closed the rendering gaps; if this port
  needs no node and no token, that is the result being reported.
- **Porting a second pane.** Exactly one, completely.

## Impact

- New code: `src/session/pane_context.rs`,
  `src/plugin/kernel_state.rs`, `src/plugin/bundled/info-panel/{plugin.toml,init.luau}`,
  `tests/bundled_info_panel.rs`.
- Changed: `src/session/mod.rs`, `src/session/plugin_manifest.rs`,
  `src/plugin/{mod.rs,capabilities.rs,discovery.rs,lifecycle.rs}`,
  `src/plugin/bundled/thurbox.d.luau`, `src/app/{mod.rs,view.rs,metrics_state.rs}`,
  `src/ui/info_panel.rs`, `tests/teardown_gate.rs`,
  `docs/{PHASE4-PANE-READINESS.md,PHASE6-TEARDOWN-READINESS.md,ARCHITECTURE.md,PERFORMANCE.md}`,
  `CLAUDE.md`.
- Feature gate: everything under `src/plugin/` is already wholly behind
  `#[cfg(feature = "plugins")]` and the new file joins it.
  `session::pane_context` is **ungated**, like `session::view_tree` after
  ADR-26 — a kernel data type gated on a Cargo feature is how you end up with
  two implementations of one pane. `tests/bundled_info_panel.rs` is
  `#![cfg(feature = "plugins")]`. `cargo tree --edges normal | grep -c mlua`
  stays 0.
- Architecture: no new edge. `session::pane_context` references only `super`;
  `plugin::kernel_state` reaches `session` only; `app` already reaches both.
  `tests/architecture_rules.rs` is unchanged — the one place that must see both
  `ui::info_panel` and `plugin::PluginHost` is an integration test, which is not
  in the library's module graph.
- Snapshots: none move. The pane's pinned frame
  (`src/ui/snapshots/thurbox__ui__info_panel__tests__info_panel_full_frame.snap`)
  is unchanged, which is the check that the `now` parameter changed no output.
