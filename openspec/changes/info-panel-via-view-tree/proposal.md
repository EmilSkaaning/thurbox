# Render the info panel through the view tree

## Why

Phase 0 of the v2 migration has one exit criterion left: *the info panel renders
through the view tree with byte-identical snapshots*. It is unmet — today
`src/ui/info_panel.rs` builds `Vec<ratatui::text::Line>` by hand and hands it to
a single `Paragraph`. `grep ViewNode src/ui/info_panel.rs` finds nothing.

The criterion is not busywork. The view tree
(`src/session/view_tree.rs`) is the contract every Phase 4 pane will be written
against, and until a *real* v1 pane is expressed in it, nobody knows whether it
can carry one. `docs/PHASE4-PANE-READINESS.md` already predicts two places
where it cannot (§3 style tokens, §4 gauges) and that audit is what this change
settles: not by argument, but by porting the pane and reporting what had to be
widened.

This is deliberately **not** making the info panel a plugin. It stays Rust,
in-process, called from `App::render_info_panel` exactly as now. The only thing
that changes is the intermediate representation: from ratatui `Line`s to a
`session::view_tree::ViewNode` rendered by `ui::plugin_pane::render_tree`. The
pane is the *test subject* for the catalogue, not a plugin.

Three catalogue gaps blocked the port and are widened here, each because the
pane cannot be drawn without it — not to make a general drawing API:

1. **Style tokens could not address the palette.** Five tokens
   (`accent`/`muted`/`danger`/`success`/`warning`) resolve onto five palette
   fields. The pane draws from eleven more *distinct* fields, and — the case
   PHASE4 §3 calls out — cannot draw a session's status in that status's colour
   at all: `status_done` and `status_unreachable` have no token, and `danger` is
   a different palette field from `status_blocked`.
2. **A gauge needs a width the tree does not carry.** `render_gauge_lines`
   right-aligns its suffix at `width - label - right` and sizes its bar to
   `width - 2`. Five of the pane's gauges are the majority of its visual
   weight. PHASE4 §4 already chose between the two closures and picked the
   node.
3. **Nothing in the catalogue wraps.** `ViewNode::Line` clips at one row by
   construction. The pane's `Paragraph` wraps with `Wrap { trim: false }`, and
   it must: `Activity` and `Signal` carry agent-supplied text of unbounded
   length, and a gauge header whose label plus suffix exceeds the width
   overflows. Clipping them would be a user-visible regression, not an
   implementation detail.

## What Changes

- `session::view_tree::StyleToken` gains eleven variants, each named for and
  resolving 1:1 onto the `ThemePalette` field it addresses.
- `session::view_tree::ViewNode` gains `Gauge` (label, percent, optional
  suffix — the kernel owns the geometry) and `Paragraph` (inline runs,
  soft-wrapped to the available width, as many rows tall as it needs).
- `ui::plugin_pane`'s height computation becomes width-aware, because a wrapped
  node's height is a function of the width it is given.
- `ui::info_panel::render_info_panel` builds one `ViewNode` and renders it
  through `render_tree`; the v1 line builders are retained **as a `#[cfg(test)]`
  oracle** and a differential test asserts the two agree cell for cell.
- Plugins reach all three widenings (`ui.gauge`, `ui.paragraph`, the new
  tokens), because a widening only closes a PHASE4 gap if a third party can use
  it.

## Capabilities

- `plugin-host/view-tree` — MODIFIED: the token set, the node catalogue, and
  the addition of a node whose height depends on its width.
- `migration/phase-0` — ADDED: the exit criterion itself, as a falsifiable
  requirement naming the snapshot.

## Non-goals

- **Making the info panel a plugin.** No Luau, no VM, no capability grant. The
  pane stays a Rust function on the UI thread.
- **A `sessions` host binding** (PHASE4 §2). The pane still receives its
  `SessionInfo` as a function argument. Nothing here lets a *plugin* read
  kernel state, so PHASE4 §2 stays open and the info panel is not yet portable
  to a plugin for that reason alone.
- **Per-pane keyboard visibility** (PHASE4 §5, still open).
- **Reporting a resolved rect back to a plugin.** Rejected in `design.md`.
- **Porting any other pane.** Exactly one is ported, which is what makes the
  finding trustworthy.

## Impact

- Code: `src/session/view_tree.rs`, `src/ui/plugin_pane.rs`,
  `src/ui/info_panel.rs`, `src/plugin/view.rs`,
  `src/plugin/capabilities.rs`, `src/plugin/bundled/thurbox.d.luau`.
- Feature gate: the two plugin-side files are already wholly behind
  `#[cfg(feature = "plugins")]`; `session::view_tree` and `ui::plugin_pane` are
  **not** gated today and stay ungated, so no gate changes.
- Architecture: no new module edge. `session::view_tree` still references only
  `super`; `ui::plugin_pane` and `ui::info_panel` still reference only
  `session` + `ui`. `tests/architecture_rules.rs` needs no edit.
- Snapshots: one new snapshot
  (`src/ui/snapshots/thurbox__ui__info_panel__tests__info_panel_full_frame.snap`),
  generated from the **v1** renderer before the port and required not to move
  after it. No existing snapshot may move.
