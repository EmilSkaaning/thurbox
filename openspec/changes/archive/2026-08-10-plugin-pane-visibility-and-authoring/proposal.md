## Why

A plugin pane exists but cannot be turned off. It is always on screen when its
plugin is running, has no toggle, and appears in no keybinding list — so the v1
behaviour every native panel has (`F2`/`F3`/`F5`/`F9` show and hide it) is the
one thing a plugin pane cannot do. `docs/v2/` calls this out as a Phase 1 exit
criterion, and it is the difference between a pane that demos and one a user
would keep installed.

Authoring is the second half of the same gap. A plugin today hand-writes raw
tables (`{ kind = "text", content = "…" }`) with no constructors and no type
definitions, so every typo is a runtime rejection rather than a checked error,
and the shipped `.luau` files are linted by nothing.

## What Changes

- **Pane visibility becomes kernel state, persisted per pane.** The manifest
  seeds it with `default_visible`; the kernel owns it thereafter and remembers
  it across restarts, so a user's show/hide choice survives a relaunch — unlike
  v1, which resets every panel's visibility on every launch.
- **A rebindable toggle.** A new `TogglePluginPane` action shows and hides the
  plugin pane, listed and rebindable in the F1 editor like every other action.
- **`@thurbox.ui` node constructors.** `ui.text`, `ui.row`, `ui.column`,
  `ui.list`, `ui.divider`, `ui.spacer` build view nodes, so a plugin writes
  `ui.text("hi", "accent")` instead of a literal table with a `kind` field it
  can misspell.
- **Luau type definitions and a linter in CI.** A shipped `.d.luau` declares
  the `@thurbox` surface, and `luau-analyze` runs in strict mode over the
  bundled plugin as a CI job — the toolchain `docs/v2/` Phase 0 asks for before
  Luau code ships, which currently checks nothing.

## Capabilities

### New Capabilities

- `plugin-host/pane-visibility`: who owns whether a pane is shown, how the
  manifest seeds it, how it persists, and what the toggle does.
- `plugin-host/authoring`: the `@thurbox.ui` constructor surface and the
  guarantees a plugin author gets from it.

### Modified Capabilities

- `plugin-host/manifest`: a pane may declare `default_visible`.
- `plugin-host/panes`: a declared pane is shown only when its visibility says
  so, rather than whenever its plugin runs.

## Non-goals

- **No per-pane generated commands.** ADR-V21's
  `<plugin>.<pane>.{toggle,show,hide}` needs the command registry, which is
  Phase 5. This change gives one kernel toggle for the plugin pane; the
  per-pane command space arrives with the registry rather than being faked now.
- **No key events into plugins.** Panes still display and do not respond;
  routing input into a VM is its own contract.
- **No hot reload.** Next change.
- **No `@thurbox/widgets`.** Higher-level widgets (`table`, `badge`,
  `keyHints`) come after the primitives have a real consumer.

## Impact

`session/plugin_manifest.rs` (the new field), `session/keybindings.rs` (a new
`Action`), `storage/` (persisted visibility), `plugin/capabilities.rs` (the
`ui` table), `app/` (toggle handling and the visibility gate), plus a new
`.d.luau` and a CI job running `luau-analyze`.

Persisting visibility is the first plugin state thurbox keeps across restarts.
It goes in the existing `metadata` table rather than a new one — it is a small
keyed value, exactly what that table is for.
