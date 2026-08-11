# Let a pane declare the action that toggles it and the feature flag that gates it

## Why

`docs/PHASE4-PANE-READINESS.md` §14's second row: **"the same toggle and the same
flag."** A native pane answers a kernel action and rides a kernel feature switch —
`Action::ToggleInfoPanel` flips `App::show_info_panel`, and `[features] info_panel =
false` hides the pane, blocks the chord and removes its footer pill. A plugin pane
has neither. Its visibility is `TogglePluginPane` (`F10`, one action for every pane,
ADR-28) plus a stored per-pane choice, and no `[features]` flag reaches it at all.

So a handed-over info panel would stop answering `F2`/`Ctrl+B` — the key a user has
for it — and `[features] info_panel = false` would stop hiding it, because the flag
gates a renderer that is no longer what draws. ADR-46 gave that pane its *seat*; a
seat whose key and switch are gone is a pane in the right place that the interface
no longer controls.

The declaration is also how the `[features]` flags eventually retire: a flag whose
only consumer is a pane can move into that pane's manifest and out of the kernel.

## What Changes

- **`[[panes]]` gains `toggle_action`.** The name of the kernel action that shows
  and hides this pane, spelled exactly as `keybindings.json` spells it
  (`toggle_action = "ToggleInfoPanel"`) — one spelling for an action everywhere.
  Validated against a **closed set**: `Action::pane_toggles()`, the six actions whose
  job is to show or hide a pane (`ToggleInfoPanel`, `ToggleFileViewer`, `FocusTasks`,
  `ToggleSessionList`, `ToggleReview`, `ToggleShell`). Anything else is a manifest
  error naming the action and listing the six — an unknown name, a real action that
  is not a pane toggle, and `TogglePluginPane` itself (the generic toggle already
  reaches every pane; binding it would toggle a pane twice).
- **`[[panes]]` gains `feature`.** The `[features]` key that gates this pane
  (`feature = "info_panel"`), validated against a closed set of the flags that
  exist — a new `session::settings::FeatureFlag`, whose wire names are
  settings.toml's own keys. An unknown flag is a manifest error naming it.
- **The bound action toggles the pane, alongside the kernel's own pane for that
  seat.** Both occupants flip, so pressing the key twice returns to where it
  started and hiding the plugin pane hands the seat back — the reversibility rule
  ADR-46 established. When the native renderer is eventually deleted its half of
  the toggle goes with it, and only the plugin's remains.
- **A gated-off pane is not a pane.** With its flag off it is not shown, not
  seated, not focusable, not rendered (its VM is not entered), and not offered by
  `F10` or its picker. It is not *forgotten*: the user's stored visibility choice
  survives, so turning the flag back on restores what they had.
- **Both fields are optional**, and a manifest that declares neither behaves
  exactly as it does today.

## Capabilities

### Modified Capabilities

- `plugin-host/manifest`: two new `[[panes]]` fields, each validated against a
  closed set, each naming the offending value when it is not in it.
- `plugin-host/pane-visibility`: a declared kernel action toggles the pane; a
  declared feature flag gates it everywhere visibility is read.

## Non-goals

- **No pane is deleted and no native renderer stops being reachable.** No bundled
  manifest declares either field: a bundled reproduction that answered `F2` would
  toggle *both* panes for every user who tried the key, which is a behaviour change
  in a change whose point is that nothing changes yet. The fields ship with a test
  plugin, ready for the handover that needs them.
- **The action's own feature gate is unchanged.** `ToggleInfoPanel` still toasts
  "Info panel is disabled" when `[features] info_panel = false`, because that is the
  kernel pane's switch. A plugin pane's availability is *its own* declared flag, so
  each occupant is gated by the flag it named.
- **A plugin cannot invent an action.** The set is the kernel's, closed, and
  curated to the actions that show or hide a pane. A plugin wanting a key of its own
  declares a `[[keybindings]]` entry (ADR-34).
- **No new flag, and no flag retires here.** `FeatureFlag` names exactly the
  `[features]` keys that exist today.
- **`F10` is not replaced.** A pane with no `toggle_action` is still toggled by the
  generic action and its picker, which stays the answer for a pane that is not
  replacing a native one.

## Impact

The plugin host ships in every install (ADR-40), so this is not behind a Cargo
feature; `--no-default-features` has no plugin panes, so no manifest field is read
and no toggle changes.

`src/session/plugin_manifest.rs` (`PaneDecl::toggle_action`, `PaneDecl::feature`,
two error variants), `src/session/keybindings.rs` (`Action::pane_toggles`),
`src/session/settings.rs` (`FeatureFlag`, `FeatureFlags::enabled`),
`src/plugin/pane.rs` (both fields, `is_enabled`/`is_shown`/`is_focusable_with`),
`src/plugin/lifecycle.rs` (carry them onto the published pane),
`src/app/mod.rs` + `src/app/key_handlers.rs` + `src/app/view.rs` +
`src/app/motion_state.rs` (the gate at every visibility read, and the action
hook), `src/app/acceptance.rs`, `docs/ARCHITECTURE.md` (ADR-47),
`docs/PHASE4-PANE-READINESS.md`, `docs/CONFIG.md`, `CLAUDE.md`.
