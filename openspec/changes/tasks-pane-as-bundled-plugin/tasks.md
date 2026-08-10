# Tasks

## 1. Emphasis on the view tree

- [x] `src/session/view_tree.rs`: `TextStyle` gains `dim: bool` and
  `underline: bool`; update every struct literal in the tree.
  Verify: `cargo nextest run -E 'test(session::view_tree)'`
- [x] `src/ui/plugin_pane.rs`: `text_style` applies `Modifier::DIM` /
  `Modifier::UNDERLINED`; a test asserts a run's emphasis reaches the buffer and
  does not leak onto its neighbour.
  Verify: `cargo nextest run -E 'test(ui::plugin_pane)'`
- [x] `src/plugin/view.rs`: convert the `underline` / `dim` booleans on a text
  node, with a test for each and for their absence.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::view)'`
- [x] `src/plugin/capabilities.rs`: `ui.text(content, style?, bold?, underline?,
  dim?)`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`

## 2. The published task section

- [x] `src/session/pane_context.rs`: `TaskSnapshot` / `TasksSnapshot`,
  `PaneContext::tasks`, and `MAX_TASK_ROWS` with its rationale. Tests: structural
  equality (the change gate depends on it) and the wire names matching
  `TaskStatus::as_str`.
  Verify: `cargo nextest run -E 'test(session::pane_context)'`
- [x] `src/session/plugin_manifest.rs`: `Capability::Tasks`, wire name `tasks`,
  in `as_str`, `all`, and `reads_kernel_state`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_manifest)'`
- [x] `src/plugin/kernel_state.rs`: `tasks_table` — an always-present table of
  `entries` plus `focused`, offsets as a 1-based array of numbers.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::kernel_state)'`
- [x] `src/plugin/capabilities.rs`: insert `tasks` under `Capability::Tasks`;
  extend the per-capability gating tests so one grant still implies no other.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`

## 3. Publish it from `app`

- [x] `src/app/view.rs` → `src/app/mod.rs`: extract `App::task_pane_entries` so
  the native pane and the publisher build the rows from one function.
  Verify: `cargo nextest run -E 'test(app::)'`
- [x] `src/app/mod.rs`: `build_pane_context` fills the task section — bounded by
  `MAX_TASK_ROWS`, empty when `features.tasks` is off, `selected` resolved from
  the focus and the search preview.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [x] `src/app/acceptance.rs`: assert the published section reflects the task
  list and its selection, is empty with the feature off, and that publishing it
  is still change-gated (no new `pane_context_publishes` on an idle tick).
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 4. The native pane renders its tree

- [x] `src/ui/highlight.rs`: `highlight_runs` — the run segmentation the span
  builders already perform, reused rather than reimplemented.
  Verify: `cargo nextest run -E 'test(ui::highlight)'`
- [x] `src/ui/tasks_panel.rs`: `TaskRow` / `VisibleRows`, `visible_rows` (window +
  fit + selection), `tasks_tree` (geometry-free), and `render_tasks_panel`
  painting the tree through `plugin_pane::render_tree` while keeping its hitboxes
  and focused footer. Keep the span-based row builder as a `#[cfg(test)]` oracle
  and assert the tree renders cell-for-cell identically.
  Verify: `cargo nextest run -E 'test(ui::tasks_panel)'`
- [x] `src/app/view.rs`: call `visible_rows`/`tasks_tree` through
  `render_tasks_panel` unchanged from the caller's side; no snapshot moves.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` shows no
  `.snap.new` under `src/`

## 5. The bundled plugin

- [x] `src/plugin/bundled/tasks/plugin.toml`: `capabilities = ["render",
  "tasks"]`, one pane `default_visible = false`.
- [x] `src/plugin/bundled/tasks/init.luau`: status → glyph and token, the
  selected > dimmed > status precedence, the UTF-8-aware match segmentation, the
  linked marker, the empty-state line.
- [x] `src/plugin/discovery.rs`: add it to `BUNDLED`.
  Verify: `cargo nextest run --features plugins -E 'test(bundled)'`
- [x] `src/plugin/bundled/thurbox.d.luau`: the task types, the reader, and the two
  new emphasis flags on `ViewNode` and `ui.text`.
  Verify: `./scripts/dev/lint-luau.sh`

## 6. Prove it renders the same pane

- [x] `tests/bundled_tasks_panel.rs` (new, `#![cfg(feature = "plugins")]`): tree
  equality against `tasks_tree` over content variants (empty, focused and not, a
  selected row, a search with dimmed and matched rows, a multi-byte title, linked
  rows, every status); the plugin declares exactly `render` + `tasks`; and the two
  geometry divergences pinned with their reasons.
  Verify: `cargo nextest run --features plugins --test bundled_tasks_panel`
- [x] `tests/teardown_gate.rs`: unchanged, and still records the tasks row as
  blocked because `src/app/view.rs` names `tasks_panel`.
  Verify: `cargo nextest run --test teardown_gate`

## 7. Docs

- [x] `docs/PHASE4-PANE-READINESS.md`: the tasks port — what sufficed, the
  emphasis widening, the two open geometry gaps with their measurements, and the
  formatter question left where it was.
- [x] `docs/ARCHITECTURE.md`: an ADR for the emphasis flags and the task section's
  division of labour (what the kernel resolves, and why not the database seam).
- [x] `CLAUDE.md`: the `tasks` capability, the bundled pane, the emphasis flags.
  Verify: `rumdl check .`

## 8. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (≥ 2154, 0 failed)
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
  (≥ 2489, 0 failed)
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `./scripts/dev/lint-luau.sh` ; `rumdl check .`
