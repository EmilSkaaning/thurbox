## 1. View tree as pure data

- [x] 1.1 Create `src/session/view_tree.rs`: the `ViewNode` enum (text, row, column, list, divider, spacer), the `StyleToken` enum, and the depth/node/text bounds as constants. Derive `PartialEq` for the change comparison (design D5). Register in `src/session/mod.rs`.
- [x] 1.2 Add text sanitization — strip control characters, truncate on a character boundary at the length bound.
- [x] 1.3 Unit tests: nesting preserved, empty container valid, unknown style token rejected, no-token default, bounds enforced, escape sequence stripped, multi-byte truncation at the boundary.

**Verify:** `cargo nextest run -E 'test(view_tree)'`

## 2. Manifest and capability changes

- [x] 2.1 Add `PaneSlot` to `src/session/plugin_manifest.rs` as a closed enum with a default; add it to `PaneDecl`.
- [x] 2.2 Add the `render` capability to the vocabulary.
- [x] 2.3 Reject a manifest declaring a pane without the render capability, at validation (design D6).
- [x] 2.4 Tests: known slot, unknown slot rejected, omitted slot defaults, pane without render rejected, pane with render valid, render without a pane valid.

**Verify:** `cargo nextest run -E 'test(plugin_manifest)'`

## 3. Lua → view tree conversion

- [x] 3.1 Create `src/plugin/view.rs` converting an `mlua::Value` into a `ViewNode`, enforcing depth and node bounds during the walk so a cycle terminates via the depth bound rather than looping.
- [x] 3.2 Return a structured error for every malformed shape — non-table result, unknown kind, unknown token, missing required field — and never panic.
- [x] 3.3 Tests for each failure mode plus the self-referential table.

**Verify:** `cargo nextest run -E 'test(plugin::view)' --features plugins`

## 4. Render request across the plugin thread

- [x] 4.1 Add a `Render { pane_id }` request to the runtime's channel; call the module's `render`, convert on the plugin's own thread (design D3), and reply with the tree or a structured error.
- [x] 4.2 Add `PluginHost::render_pane` returning the result, and hold the render capability check so a plugin without it is never asked.
- [x] 4.3 Tests: successful render, render raising, render returning an invalid tree, render exceeding the budget, plugin without the capability never asked.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 5. Pane state and the async cycle

- [x] 5.1 Add a `PanePresentation` (loading / ready(tree) / stale-with-error / failed) in `src/plugin/`, holding the last good tree and the current error.
- [x] 5.2 Drive re-render off the UI thread and apply results on the existing tick, marking dirty only when the tree differs (design D5).
- [x] 5.3 Tests: first render pending shows loading, failure keeps the last tree with an indicator, first-render failure shows failed, unchanged tree does not dirty, changed tree does.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 6. Rendering and layout

- [x] 6.1 Create `src/ui/plugin_pane.rs` rendering a `ViewNode` into a ratatui area, resolving style tokens against the active theme palette.
- [x] 6.3 Token resolution is tested against a dark (`Default`) and a light (`Catppuccin Latte`) palette, including that a token *changes* colour across the two — the property that makes tokens worth having. The layout-threshold half moves with 6.2.
- [x] 6.2 Done, after `layout-slot-model` landed the params struct and the ordered right-column occupant list. The pane is a `RightSlot::Plugin` entry rather than another positional argument.

**Verify:** `cargo nextest run -E 'test(layout)'`, `cargo nextest run -E 'test(plugin_pane)'`

## 7. Wiring and the first bundled plugin

- [x] 7.1 No allowlist change was needed: `app` is in `EXEMPT` (the coordinator imports everything by design), so `app → plugin` is already permitted. The half that matters was verified instead — `ui` contains no `crate::plugin` reference, so the renderer has no path back to a VM (design D1/D7).
- [x] 7.2 `App` holds `plugin_panes` plus a `PluginUiEvent` receiver; `main` owns a render worker that owns the host, so the UI thread never calls a plugin. `App::view` draws the pane from the cached tree.
- [x] 7.3 Added the bundled `hello` plugin (`src/plugin/bundled/hello/`), embedded with `include_str!` and materialized to `~/.local/share/thurbox/builtin-plugins/` — mirroring how built-in extensions already ship. Verified drawing in a live TUI.
- [x] 7.4 Three acceptance tests on real frames: loading → rendered → stale-with-error, no-panes leaves the layout byte-identical, and an unchanged pane set reports no change.

**Verify:** `cargo nextest run --all --features plugins`

## 8. Close-out

- [x] 8.1 `cargo nextest run --all` → **1978 passed**, 0 failed. `--features plugins` → **2120 passed**, 0 failed. Clippy clean with the feature, without it, and with `--all-features`; rustdoc, rumdl and `cargo fmt --check` clean. Clippy caught a real smell in the first draft of the renderer — `height_of` took a `width` it never used, because text does not wrap — removed rather than silenced.
- [x] 8.2 Enforced at two levels and tested at both: `PluginPane::apply` reports "changed" only when the presentation differs, and `App::set_plugin_panes` compares the whole pane set before touching `needs_redraw`. An acceptance test asserts an identical set reports no change.
- [x] 8.3 `CLAUDE.md` and `docs/CONFIG.md` updated with the plugin model, the bundled plugin, and both plugin directories.

## 9. Status

**Complete.** The plugin contract and its mounting both landed: a Luau plugin
declares a pane in its manifest, returns a view tree from `render`, and the
kernel draws it in the right-hand column — verified in a live TUI, not only in
tests. `layout-slot-model` was split out first so the pane could be a slot
entry rather than a tenth positional argument.