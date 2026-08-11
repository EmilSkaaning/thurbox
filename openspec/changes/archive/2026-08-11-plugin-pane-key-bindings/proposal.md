# Give a plugin pane its own rebindable keys

## Why

Phase 4 has reproduced five native panes as bundled plugins. Every one of them is
**read-only**: not one declares `input`, because the input model that landed with
`plugin-host/input` is too thin to reproduce a native pane's keyboard.

The v1 behaviour that has no v2 equivalent is `KeyContext`. A native pane's keys
are *scoped*: `session::Action::TasksNext` is bound to a plain `j`,
`FileViewerDown` is bound to the same plain `j`, and
`KeyBindings::lookup_in(KeyContext::Tasks, …)` keeps them apart — so every pane
gets the whole single-letter alphabet, and every one of those keys is rebindable
in the F1 editor and persisted to `keybindings.json`. A plugin pane has none of
that. It is handed a **raw key name** (`plugin_key_name` → `"j"`), it decides for
itself what that means, and there is no way for a user to rebind it, no row for it
in F1, and no record of it in `keybindings.json`.

Manifests already carry `[[keybindings]]` entries — `KeybindingDecl { id, chord }`
— and they are inert data: nothing reads them. `plugin-command-registry`'s design
§7 recorded exactly what wiring them up needs, and this change is that list:

1. `KeyBindings` resolving a chord to *either* an `Action` or a plugin binding;
2. an open key context, since a pane's scope is `pane:<id>` and the enum is
   closed;
3. the asymmetric conflict rule — a user's rebind **steals** a chord, a plugin's
   manifest default is **dropped** on collision and reported. v1 has only the
   stealing half.

Without this a pane replacement cannot meet its second condition ("its keys are
rebindable and appear in the F1 editor"), and every plugin pane would hard-code
its own keyboard — the one part of the interface users insist on owning.

## What Changes

- **A plugin binding is a keymap entry.** `KeyBindings` gains a second table,
  keyed by `(plugin, pane, binding id)`, alongside the closed `Action` map. One
  keymap, one file, one editor.
- **Its context is its pane.** A binding is active only while *that* pane is
  focused, so two plugin panes may both bind `j`, and a plugin `j` never collides
  with the tasks pane's `j`. The scope comes from the binding's own address
  rather than from a new enum variant.
- **Manifest `[[keybindings]]` becomes real.** The declaration widens to
  `{ id, pane, title?, chord? }`: it names the pane it is scoped to, its chord is
  parsed against the keymap's grammar at manifest validation, and it requires the
  `input` capability — a keybinding that could never be delivered is a manifest
  error naming its own fix, as `PaneWithoutRender` already is for panes.
- **Rebindable and visible.** The F1 editor lists one section per plugin pane
  under the kernel's sections; `r` captures a chord, `d` restores the manifest
  default, `Shift+D` resets everything. A rebind persists to `keybindings.json`
  under a `plugin:<plugin>.<pane>.<id>` key and survives a restart, a reload, and
  the plugin changing its own default.
- **A manifest default is dropped, never stolen.** If a declared chord is already
  bound to an overlapping action or to another binding of the same pane, the
  plugin's default is dropped, the binding stays unbound, and the drop is
  reported by `thurbox-cli plugin doctor`. A *user's* rebind still steals, in
  either direction.
- **Delivery carries the binding.** When a chord resolves to a pane binding the
  host passes its id alongside the raw key, so a plugin switches on
  `"delete-task"` rather than on `"d"` and a rebind needs no plugin change.

## Capabilities

### New Capabilities

- `plugin-host/pane-keys`: what a pane binding is, the scope it is active in, how
  a chord resolves to one, the collision rule, what the F1 editor shows, and how
  a rebind is persisted.

### Modified Capabilities

- `plugin-host/manifest`: the keybinding declaration — its fields, the pane it
  must name, the capability it requires, and the chord grammar it is validated
  against.
- `plugin-host/input`: a delivered key carries the binding it resolved to, when
  it resolved to one.

## Non-goals

- **No pane is ported.** No bundled plugin declares `input` or a keybinding in
  this change; the five reproductions stay read-only and the native panes stay on
  screen. `tests/teardown_gate.rs` is untouched, and every insta snapshot must be
  byte-identical.
- **No global plugin chords.** A binding fires only while its own pane is
  focused. A plugin cannot bind a chord that works anywhere — that is
  ADR-V21's generated-command surface, which exists headlessly and stays there.
- **No new capability.** `input` already gates receiving keys; this changes what
  the host says when it delivers one, not who may receive one.
- **No mutating power.** A binding tells a plugin its key fired. What a plugin
  may *do* about it is the capability question, and it is a separate change.
- **No mouse.** Keys only, as `plugin-host/input` already says.
- **No chord grammar of its own.** Plugin chords are parsed by
  `KeyChord::parse`, so `ctrl+d`, `f7` and `shift+j` mean in a manifest exactly
  what they mean in `keybindings.json`.

## Impact

Behind the existing `plugins` Cargo feature for everything that runs plugin code;
the keymap half is **ungated**, because `session::KeyBindings` is pure data that a
default build parses (an override file naming a plugin binding must round-trip in
a build with no plugin host, or a user losing the feature would silently lose
their bindings file).

`src/session/keybindings.rs` (the second table, resolution, persistence),
`src/session/plugin_manifest.rs` (the widened declaration and its validation),
`src/plugin/keymap.rs` (new: manifests → declarations), `src/plugin/lifecycle.rs`
(publish them with the panes), `src/plugin/runtime.rs` (`onKey`'s third
argument), `src/app/mod.rs` + `src/app/key_handlers.rs` (registration, routing,
the F1 editor's rows), `src/app/view.rs` (the editor's sections),
`src/cli/plugins.rs` (`doctor`'s keybinding section),
`src/plugin/bundled/thurbox.d.luau`, `docs/ARCHITECTURE.md` (ADR-34),
`docs/CONFIG.md`, `CLAUDE.md`.
