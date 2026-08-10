## Why

`docs/v2/` Phase 1 exits when "a hello-world plugin renders a pane, **receives
key events**, and **hot-reloads on save**". Rendering and the pane toggle are
done; the other two are not. Without them a plugin pane is a read-only widget
that requires a full restart to change, which is not the "write a pane in an
afternoon" loop v2 exists for.

## What Changes

- **A plugin pane can be focused and receive keys.** It joins the pane focus
  ring, and while focused its keys are handed to the plugin, which reports
  whether it consumed them. Unconsumed keys fall back to thurbox.
- **Input is a declared capability.** A plugin that did not ask for `input` is
  never handed a key, and its pane is not focusable.
- **Hot reload.** A plugin's source is watched; a change reloads it in place
  with a fresh VM, keeping its pane and visibility. `thurbox-cli plugin reload`
  does the same on demand.
- **Reload is safe by construction.** It reuses the lifecycle's existing
  return-to-`discovered` transition, so a reloaded plugin gets a genuinely new
  VM with no state carried across.

## Capabilities

### New Capabilities

- `plugin-host/input`: how a pane receives keys, what a plugin returns, and
  what happens to keys it does not consume.
- `plugin-host/reload`: what reloading does, what survives it, and how a
  failed reload behaves.

### Modified Capabilities

- `plugin-host/capabilities`: adds the `input` capability.
- `plugin-host/cli`: adds the `reload` verb.

## Non-goals

- **No mouse.** Keys only.
- **No key remapping by plugins.** A plugin sees keys while focused; it cannot
  register global chords. That needs the command registry.
- **No partial reload.** A reload rebuilds the whole VM rather than swapping
  functions.

## Impact

`session/plugin_manifest.rs` (capability), `plugin/runtime.rs` (key request),
`plugin/lifecycle.rs` (reload), `app/` (focus ring + routing), `cli/plugins.rs`
(verb), and the render worker (source watching).
