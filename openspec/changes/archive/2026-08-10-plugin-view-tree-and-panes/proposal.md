## Why

Plugins run, report their state, and can be inspected — but they cannot draw
anything. A manifest's `[[panes]]` is still only data the host enumerates, so
the thing v2 exists for (a user adding a pane without recompiling thurbox) is
still impossible.

Every v1 pane is compiled in, and that is the tax v2 is paying down: adding one
means editing `App`'s field list, `InputFocus`, `PanelAreas`, `ClickAction`,
`SettingsField`, `focus_ring`, and the pinned snapshots in lockstep. This change
introduces the contract that lets a plugin contribute a pane instead — a
declarative view tree the kernel renders, with no plugin code anywhere near the
frame.

## What Changes

- **A view tree**: a small, frozen catalog of layout and content nodes a plugin
  returns from `render`. Text, rows, columns, lists, dividers, spacers — enough
  to build the panes thurbox actually has, and deliberately not a general
  drawing API.
- **Theme tokens instead of colors.** A node styles itself with a named token
  (`accent`, `muted`, `danger`, …) that resolves against the active thurbox
  theme. A plugin cannot name an RGB value, so every plugin follows a theme
  switch for free and no plugin can render unreadable text on a light palette.
- **A pane slot.** A manifest's `[[panes]]` entry becomes a real pane in the
  right-hand column, alongside the file viewer and tasks panel, subject to the
  same width rules.
- **Rendering is asynchronous and cached.** The kernel never calls a plugin
  during a frame. A plugin's `render` runs on its own thread; the kernel keeps
  the last tree it returned and paints that. A slow plugin makes its pane
  stale, never the UI slow.
- **A failing plugin degrades to its pane, not the app.** A `render` that
  errors, times out, or returns something that is not a view tree leaves the
  last good tree on screen with an error indicator, and never blanks or panics
  the frame.
- **The bundled source gets its first plugin** — a small built-in pane used to
  dogfood the contract end to end, so the API is exercised by something that
  ships rather than only by tests.

## Capabilities

### New Capabilities

- `plugin-host/view-tree`: the node catalog, the styling model, the value
  contract a plugin's `render` must satisfy, and what happens when it does not.
- `plugin-host/panes`: how a declared pane becomes a rendered one — slot
  placement, visibility, the async render cycle, staleness, and error display.

### Modified Capabilities

- `plugin-host/manifest`: a `[[panes]]` entry gains the fields the pane model
  needs (its slot), and the rules for what a pane may declare.
- `plugin-host/capabilities`: adds the capability that lets a plugin be asked
  to render, so a plugin that does not draw cannot be handed a render request.

## Non-goals

- **No input.** Panes are read-only in this change: no focus, no keybindings,
  no mouse, no plugin-handled events. A plugin pane displays; it does not
  respond. Input is the next change and is a larger contract than drawing.
- **No overlays or modals.** Plugins draw inside their pane's rectangle only.
- **No animation.** A tree is static until the plugin returns a new one.
- **No plugin-controlled layout.** A plugin picks a slot; the kernel decides
  geometry, exactly as it does for the native panels.
- **No migration of a v1 pane.** The tasks panel, file viewer, and code review
  stay compiled in. Proving the contract can carry them is a later change, and
  doing it now would couple the first draft of the API to the hardest cases.

## Impact

**Code.** New `src/session/view_tree.rs` (pure data, so `ui` can render it
without importing `plugin` — the same split `session::review` already uses for
diffs), a Lua→tree conversion in `src/plugin/`, a renderer in `src/ui/`, and
the pane's place in `compute_layout`. `App` gains the cached trees and the
plumbing to request a re-render.

**Architecture.** This is the first change where `app` holds plugin state, and
where the render path depends on a plugin having produced something. The
allowlist gains `app → plugin`; `ui` still never sees `plugin`, only the
view-tree types in `session`.

**Performance.** The demand-driven loop is the constraint: a pane must mark the
UI dirty when its tree changes and must not force a repaint otherwise. Idle
paint rate with a plugin pane open must stay at the existing floor.

**Docs.** `CLAUDE.md`'s layout and architecture sections, plus the plugin
directory documentation, gain the pane and view-tree model.
