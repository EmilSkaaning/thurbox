## 1. Input capability and key transport

- [x] 1.1 Add `Capability::Input` to `src/session/plugin_manifest.rs`.
- [x] 1.2 Add a `Key` request to `src/plugin/runtime.rs` calling the module's `onKey(paneId, key)` and returning whether it was consumed.
- [x] 1.3 Add `PluginHost::send_key` gated on the input capability, with a bounded wait (design D1).
- [x] 1.4 Tests: consumed, not consumed, missing handler, raising handler, hanging handler bounded, plugin without the capability refused.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 2. Focus and routing

- [x] 2.1 Add `InputFocus::PluginPane` and include it in the focus ring only when a visible pane's plugin declared input.
- [x] 2.2 Route keys to the plugin while focused; fall through on unconsumed; never offer focus/quit chords (design D2).
- [x] 2.3 Tests: ring skips a pane without input, includes one with it, consumed key does not reach thurbox, unconsumed does, escape chords always work.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 3. Reload

- [x] 3.1 Add `PluginHost::reload(name)` over the existing `reset` + start path (design D3).
- [x] 3.2 Preserve pane visibility across a reload.
- [x] 3.3 Tests: source change reflected, no state survives, hidden stays hidden, failed reload recorded, recovery, other plugins unaffected.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 4. Source watching

- [x] 4.1 Record each loaded plugin's entry mtime and reload on change, on the render worker's cycle (design D4).
- [x] 4.2 Test the change-detection helper directly (changed vs unchanged).

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 5. CLI

- [x] 5.1 Add `plugin reload [<name>]`, erroring on an unknown name.
- [x] 5.2 Tests for named, unknown, and all.

**Verify:** `cargo nextest run -E 'test(cli::plugins)' --features plugins`

## 6. Close-out

- [x] 6.1 `cargo nextest run --all` -> 1990 passed; `--features plugins` -> 2157 passed. Clippy clean in both configurations, rustdoc/rumdl/fmt clean, luau-analyze strict clean.
- [x] 6.2 Verified live. Hot reload: edited a running plugin's source and the pane changed from VERSION ONE to VERSION TWO with no restart. Input: Ctrl+L focused the pane, three j presses drove the plugin's own counter 0 -> 3, footer showed Plugin.
- [x] 6.3 CLAUDE.md documents the input capability, focus behaviour, hot reload, and the reload verb.

## 7. Phase 1 exit criteria

`docs/v2/` MIGRATION Phase 1 exits when "a hello-world plugin renders a
right-slot pane, receives key events, and hot-reloads on save; its pane toggles
from a rebindable chord and its keys appear in F1".

All but the last clause are done and verified live. **A plugin's own keys do
not appear in F1**: F1 lists the kernel's `Action` enum, and per-plugin key
entries need the command registry (Phase 5). The plugin *pane toggle* is in F1
and rebindable, which is the part that does not depend on the registry.
