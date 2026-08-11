# Tasks

## 1. The two closed sets

- [x] `src/session/keybindings.rs`: `Action::pane_toggles()` — the six actions whose
      job is showing or hiding a pane, with the exclusions (the generic plugin-pane
      toggle, global search, the modal/overlay openers) stated as why.
- [x] `src/session/settings.rs`: `FeatureFlag` (wire names = the `[features]` keys)
      + `FeatureFlags::enabled(flag)`.
- [x] Unit tests: every `pane_toggles()` member is a `Global` action and the generic
      toggle is not among them; a settings file setting every `FeatureFlag::all()`
      name to `false` parses to a `FeatureFlags` with every field false (the
      exhaustiveness guard).
- [x] Verify: `cargo nextest run -E 'test(pane_toggles) + test(feature_flag)'`.

## 2. The manifest fields

- [x] `src/session/plugin_manifest.rs`: `PaneDecl::toggle_action: Option<Action>`,
      `PaneDecl::feature: Option<FeatureFlag>`, plus
      `ManifestErrorKind::{PaneToggleAction, DuplicateToggleAction}` whose messages
      list the accepted actions.
- [x] Unit tests: a valid binding parses; an unknown action name, a real
      non-toggle action, the generic toggle, a duplicate action, and an unknown
      feature are each errors naming the offending value; both fields absent
      validates.
- [x] Verify: `cargo nextest run -E 'test(plugin_manifest)'`.

## 3. The pane carries them, and the gate is one predicate

- [x] `src/plugin/pane.rs`: `toggle_action`, `feature`, and
      `is_enabled`/`is_shown`/`is_focusable_with` replacing `is_focusable`.
- [x] `src/plugin/lifecycle.rs`: `panes()` copies both off the manifest.
- [x] Verify: `cargo nextest run -E 'test(pane) + test(lifecycle)'`.

## 4. The gate at every visibility read, and the action hook

- [x] `src/app/mod.rs`: `plugin_seat`, `visible_plugin_panes`,
      `focusable_plugin_pane`, `toggle_plugin_pane`, `open_plugin_panes_picker`,
      `sync_plugin_panes_picker` and `publish_plugin_pane_visibility` read
      `is_shown`/`is_enabled` rather than `visible`.
- [x] `src/app/view.rs`, `src/app/motion_state.rs`: same, so a gated-off pane is
      neither painted nor granted a motion lease.
- [x] `src/app/key_handlers.rs`: `dispatch_action` toggles every pane bound to the
      action before running the kernel's own dispatch.
- [x] Verify: `cargo nextest run -E 'test(plugin_pane)'`.

## 5. Acceptance

- [x] `src/app/acceptance.rs`: the declared action shows and hides the pane and
      leaves the kernel's own pane toggling too; a gated-off pane is not drawn, not
      seated, not focusable, published hidden, and not offered by the generic
      toggle; the switch coming back on restores the user's choice.
- [x] Verify: `cargo nextest run -E 'test(bound) + test(gated)'`.

## 6. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-47.
- [x] `docs/PHASE4-PANE-READINESS.md`: §14's toggle-and-flag row closes; §22 records
      what it closed and what it did not.
- [x] `docs/CONFIG.md`: the manifest row names the two fields.
- [x] `CLAUDE.md`: the plugin-host paragraph.
- [x] Verify: `rumdl check .`.

## 7. Whole-tree verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
- [x] By hand: a throwaway plugin in the sandbox declaring
      `toggle_action = "ToggleInfoPanel"` and `feature = "info_panel"` — `F2` shows
      and hides it, and `[features] info_panel = false` takes it off screen.
