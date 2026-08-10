## 1. Manifest

- [x] 1.1 Add `default_visible: bool` (defaulting to true) to `PaneDecl` in `src/session/plugin_manifest.rs`.
- [x] 1.2 Tests: omitted defaults to visible, explicit false records it.

**Verify:** `cargo nextest run -E 'test(plugin_manifest)'`

## 2. Persisted visibility

- [x] 2.1 Add `plugin_pane_visible.<plugin>.<pane>` accessors to `src/storage/` over the existing `metadata` table (design D2).
- [x] 2.2 Resolution helper: stored value wins, else the manifest seed.
- [x] 2.3 Tests: absent falls back to the seed, stored true/false wins, round-trips.

**Verify:** `cargo nextest run -E 'test(plugin_pane_visible)'`

## 3. Toggle action

- [x] 3.1 Add `Action::TogglePluginPane` to `src/session/keybindings.rs` with a default chord, in `rebindable_in_order()` and a help section.
- [x] 3.2 Handle it in `App::dispatch_action`: flip the first plugin pane's visibility, persist it, mark dirty.
- [x] 3.3 Gate `App::show_plugin_pane` and the rendered pane on visibility.
- [x] 3.4 Tests: toggle hides/shows, no panes is a no-op, hidden pane means a layout identical to no panes, the action is listed as rebindable.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 4. Authoring surface

- [x] 4.1 Add a `ui` table to the host module in `src/plugin/capabilities.rs` with constructors for every node kind, ungated (design D3).
- [x] 4.2 Rewrite the bundled `hello` plugin to use the constructors.
- [x] 4.3 Tests: constructors present without capabilities, each kind round-trips through conversion, an unknown style token still fails at conversion.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 5. Luau toolchain

- [x] 5.1 Add `src/plugin/bundled/thurbox.d.luau` declaring the `@thurbox` surface.
- [x] 5.2 Add a CI job installing and running `luau-analyze` in strict mode over `src/plugin/bundled/`.
- [x] 5.3 Add a `just lint-luau` target that skips with a notice when the binary is absent (design D4).
- [x] 5.4 Confirm the bundled plugin type-checks.

**Verify:** `luau-analyze --mode strict src/plugin/bundled/hello/init.luau`

## 6. Close-out

- [x] 6.1 `cargo nextest run --all` → **1990 passed**; `--features plugins` → **2142 passed**. Clippy clean with and without the feature, rustdoc/rumdl/fmt clean, `luau-analyze --mode=strict` clean, shellcheck clean. The `Action` variant-count guard test caught the new action and was updated (61 → 62) — the parallel-table tax v2 exists to remove, working as designed in the meantime.
- [x] 6.2 Verified live: the pane renders, `F10` hides it and the terminal reclaims the width, and `metadata` holds `plugin_pane_visible.hello.hello|0` afterwards.
- [x] 6.3 `CLAUDE.md` keybinding table gained `F10`; the plugin section documents `ui.*`, theme tokens, kernel-owned visibility, and the Luau toolchain. `docs/CONFIG.md` notes the persisted visibility.

## 7. Notes

See `notes.md` for the recorded deviation from ADR-V21 and the Luau toolchain
limitation this change works around.
