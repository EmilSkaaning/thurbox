# Anchored overlays instead of a floating-element ban

## Why

v1 already ships a floating element, as a special case.
`render_compose_inline` in `src/ui/code_review.rs` floats the comment compose
box at the selected diff line — prefer below, else above, else pin to the bottom
edge, inset one column — in eleven lines local to that one function, reachable
by nothing else. A dropdown, a context menu or a tooltip would rewrite them with
its own edge cases.

The v2 design first answered this with a `diff.inlineAt` slot on the diff node,
which blesses the one case v1 had already built. Retiring it **before** anything
depends on it is what stops a pane being migrated twice.

The general answer is an **anchor**: a rect positioned against another rect
rather than by the split flow, resolved after the base tree and drawn into a
per-pane overlay layer. The compose box becomes its first consumer instead of
its own implementation of it.

## What Changes

- **New**: an anchor spec plus a pure resolver — given a target rect, a clip
  rect and a desired size, produce the placed rect, choosing the requested
  side, flipping to the opposite side when the requested one has no room, and
  docking to the clip's far edge when neither side fits or the target is not on
  screen.
- **New**: a per-pane overlay layer. Overlays are recorded in declaration
  order, clipped to the pane, and reported topmost-first so hit-testing runs
  the overlay before the base layer.
- **Replaced**: `render_compose_inline`'s bespoke placement arithmetic. The
  compose box declares an anchor against the selected diff row's rect and the
  layer places it. The resulting rect is byte-identical to what the special
  case produced, in all three of its branches.
- **Fixed as a consequence**: a click that lands on the compose box no longer
  falls through to the diff row underneath it (which used to move the
  selection while the box kept commenting on the original line).
- **Unchanged**: the invariant that the *base* layer never overlaps, and that
  exactly one pane holds focus — an overlay belongs to its pane and is not a
  focus target.

## Capabilities

- `layout/overlay` (new) — the anchor spec, its resolution rules, the
  degradation order, and the overlay layer's ordering and hit-test priority.
- `layout/workspace-tree` (modified) — the tree's "sibling regions never
  overlap" requirement is restated as applying to the base layer, with the
  overlay layer named as the ordered exception.

## Non-goals

- **No `layout.toml` anchor syntax and no plugin-facing `anchor` node prop.**
  Both need a node-id space inside a pane, and on this branch a pane's contents
  are Rust render functions with no ids to anchor to. The spec covers the
  mechanism and the one native consumer; the `to = "<id>"` lookup lands with
  the plugin node tree.
- **No anchor nesting.** The cap of three exists to bound the pass count for
  menus anchored to menus, which requires the same node-id space. Nothing here
  can nest, so nothing here enforces a cap.
- **No escaping the pane rect.** An overlay is clipped to its pane. Crossing
  into a neighbour needs cross-pane z-ordering and an answer for what happens
  when the owning pane is hidden mid-interaction.
- **No `z-index`.** Order is positional: declaration order within a pane.
- **No new overlays.** Exactly one surface is ported. Migrating other modals
  onto the layer is separate work with its own snapshots to defend.

## Impact

- `src/session/overlay.rs` (new) — the spec types and the resolver.
- `src/ui/overlay.rs` (new) — the per-pane overlay layer.
- `src/ui/code_review.rs` — `render_compose_inline` reduced to a declaration;
  `CodeReviewHits` reports the overlay rects.
- `src/app/mod.rs` — records the overlay rects as click targets ahead of the
  diff rows.
- `tests/architecture_rules.rs` — **unchanged**. Its allowlist is per
  *top-level* module, and both new files sit inside modules it already governs:
  `session::overlay` references no crate module (`session`'s allowance is empty)
  and `ui::overlay` references `session`, which `ui` may already do. **No new
  architectural edge is introduced**, so `CLAUDE.md`'s dependency block and
  `docs/CONSTITUTION.md` §2 are untouched on that account.
- Docs updated in the same change: `CLAUDE.md` (the `ui/` architecture bullet
  and the code-review comments bullet) and `docs/ARCHITECTURE.md` (a new ADR).
  `docs/PHASE4-PANE-READINESS.md` is **not** touched: its five gaps are about
  what a *plugin* pane cannot express, and none of them is a floating element.

## Gate

**No new compile-time gate.** This is kernel layout code on the frame path for
every build, exactly like the workspace tree it resolves against
(`layout/workspace-tree`), and it replaces shipping v1 code rather than adding
a parallel v2 path. The plugin-facing surface that *will* be gated behind the
`plugins` feature is the `anchor` node prop, which this change deliberately
does not add.
