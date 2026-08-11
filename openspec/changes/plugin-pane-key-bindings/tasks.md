# Tasks

## 1. The keymap's second table

- [x] `src/session/keybindings.rs`: `PaneBindingId` (`plugin`/`pane`/`id`, with
      `qualified()` and `parse` over `<plugin>.<pane>.<id>`), `PaneBinding`
      (title, manifest default, current chords), `PaneBindingDecl` (what a
      manifest contributes), and `BindingTarget` with `label()`.
- [x] `src/session/keybindings.rs`: `KeyBindings::{register_pane_bindings,
      pane_bindings, pane_sections, chords_for_pane, lookup_pane_binding,
      rebind_pane, reset_pane}`; extend `rebind` to steal from an overlapping pane
      binding and return `BindingTarget`; extend `conflict_warnings`.
- [x] `src/session/keybindings.rs`: persist under `plugin:<plugin>.<pane>.<id>` in
      `to_json`/`from_json_with_warnings`, keeping an entry whose plugin is absent
      as an override rather than dropping it.
- [x] Verify: `cargo nextest run -E 'test(pane_binding)'` and
      `cargo nextest run -E 'test(keybinding)'`.

## 2. The manifest declaration

- [x] `src/session/plugin_manifest.rs`: widen `KeybindingDecl` to
      `{ id, pane, title?, chord? }`; validate the pane exists, the chord parses,
      and the `input` capability is requested — new `ManifestErrorKind` variants
      naming the binding; correct the stale "does not own the chord grammar"
      comment.
- [x] Verify: `cargo nextest run -E 'test(plugin_manifest)'`.

## 3. Manifests → declarations → the keymap

- [x] `src/plugin/keymap.rs` (new): `pane_bindings_for(plugins)` building
      `PaneBindingDecl`s from discovered manifests, skipping a plugin not granted
      `input`; module registered in `src/plugin/mod.rs`.
- [x] `src/plugin/lifecycle.rs`: `PluginHost::pane_bindings()` over running
      plugins, using each slot's *granted* set.
- [x] `src/app/mod.rs`: `PluginUiEvent::Panes` carries the declarations;
      `set_plugin_panes` registers them and logs any dropped default.
- [x] `src/main.rs`: send them with the panes at startup and after a reload.
- [x] Verify: `cargo nextest run --features plugins -E 'test(keymap)'`,
      `cargo nextest run --features plugins -E 'test(pane_bindings)'`.

## 4. Delivery: the binding rides with the key

- [x] `src/plugin/runtime.rs`: `on_key(pane, key, binding)` passing a third
      argument to `onKey`; `src/plugin/lifecycle.rs`: `send_key` takes the binding.
- [x] `src/app/mod.rs`: `PluginKeyRequest.binding`; `src/app/key_handlers.rs`:
      `handle_plugin_pane_key` resolves the focused pane's binding first and
      delivers both.
- [x] `src/main.rs`: forward the binding to `send_key`.
- [x] `src/plugin/bundled/thurbox.d.luau`: `onKey`'s third parameter.
- [x] Verify: `cargo nextest run --features plugins -E 'test(on_key)'`,
      `./scripts/dev/lint-luau.sh`.

## 5. The F1 editor

- [x] `src/app/key_handlers.rs`: `handle_help_key` operates on
      `BindingTarget`s (`App::help_targets`), so capture / `d` / `Shift+D` reach a
      pane binding; toast the reassignment with `BindingTarget::label`.
- [x] `src/app/view.rs`: `build_rebindable_rows` renders the pane sections after
      the kernel's, titled `<plugin> · <pane> (when focused)`.
- [x] Verify: `cargo nextest run -E 'test(help)'`, and
      `cargo nextest run --all` for the insta snapshots (the F1 snapshot must not
      move: no plugin declares a binding).

## 6. Reporting

- [x] `src/cli/plugins.rs`: `doctor` gains a keybindings section — every declared
      binding with its effective chord, or the reason it has none — built without
      starting a VM.
- [x] Verify: `cargo nextest run --features plugins -E 'test(doctor)'`, and by
      hand: `cargo run --features plugins --bin thurbox-cli -- plugin doctor`.

## 7. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-34 — a pane binding is addressed, not
      enumerated; the asymmetric collision rule; why not the command registry.
- [x] `docs/CONFIG.md`: the `plugin:` key shape in `keybindings.json`.
- [x] `CLAUDE.md`: the plugin-host paragraph gains the pane-keys sentence.
- [x] Verify: `rumdl check .`.

## 8. Whole-tree verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
- [x] By hand: `scripts/dev/sandbox.sh --fresh --plugins --show hello` with a
      throwaway plugin declaring a binding, driven by `tmux send-keys`.
