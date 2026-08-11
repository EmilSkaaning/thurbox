# Tasks

## 1. Rows out of the renderer

- [x] `src/ui/plugin_pane.rs`: `render_tree` returns the outermost list's row
      hitboxes (`Vec<ui::RowHitbox>`, 1-based, list-space indices that account for
      the kernel's scroll window); unit tests for a plain list, a scrolled list, a
      list nested in a column, a nested list contributing nothing, and a tree with
      no list.
- [x] Verify: `cargo nextest run --features plugins -E 'test(plugin_pane)'`.

## 2. Recording and dispatching the click

- [x] `src/app/mod.rs`: `ClickAction::PluginPaneRow { plugin, pane, row }`.
- [x] `src/app/view.rs`: `render_plugin_panes` records the row targets, then the
      pane's whole-rect `FocusPane` fallback — rows first, so an on-row click wins.
- [x] `src/app/key_handlers.rs` / `src/app/mod.rs`: dispatch focuses the clicked
      pane and offers the click to its plugin.
- [x] Verify: `cargo nextest run --features plugins -E 'test(click)'`.

## 3. Focus that names its pane

- [x] `src/app/mod.rs`: `focused_plugin_pane: Option<(String, String)>`, consulted
      by `focusable_plugin_pane` and validated on every read so a vanished pane
      cannot hold focus; cleared when it stops being focusable.
- [x] Verify: `cargo nextest run --features plugins -E 'test(focus)'`.

## 4. Delivery

- [x] `src/app/mod.rs`: the key request becomes an input request carrying either a
      key (with its binding) or a click.
- [x] `src/plugin/runtime.rs`: `on_click(pane, row)` → `onClick`;
      `src/plugin/lifecycle.rs`: `send_click`, refused without `input`.
- [x] `src/main.rs`: serve the click on the same channel as a key.
- [x] `src/plugin/bundled/thurbox.d.luau`: `onClick`.
- [x] Verify: `cargo nextest run --features plugins -E 'test(on_click) + test(lifecycle)'`,
      `./scripts/dev/lint-luau.sh`.

## 5. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-36 — a click is a row, focus names its pane, no
      geometry crosses, and what is left open.
- [x] `docs/PHASE4-PANE-READINESS.md`: the click is no longer among the things a
      pane port leaves in the kernel; the wheel and the scrollbar still are.
- [x] `CLAUDE.md`: the plugin-host paragraph.
- [x] Verify: `rumdl check .`.

## 6. Whole-tree verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
- [x] By hand: a throwaway plugin pane in the sandbox, clicked by injecting the
      SGR mouse sequence (`tmux send-keys -l $'\033[<0;col;rowM'` — `send-keys -M`
      only forwards an event a binding already received), reporting `board #3`,
      `board #5`, and nothing new for a click below the last row.
