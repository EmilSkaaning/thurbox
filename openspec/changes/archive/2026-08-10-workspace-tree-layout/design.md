# Design — workspace tree layout

## Context

See `proposal.md` — Why. The constraint that shapes every decision below is that
**~115 pinned `insta` acceptance snapshots must not move**. They encode the exact
output of four hand-written `ratatui::layout::Layout::split` calls in
`src/ui/layout.rs`, including ratatui's own rounding when percentages do not
divide evenly. Anything that re-derives that arithmetic is a risk; anything that
reuses it is safe by construction.

## Shape

Three stages replace one function body:

```text
LayoutParams ──preset──▶ workspace tree ──solve──▶ placements ──project──▶ PanelAreas
              (ui)        (session data)   (ui)     Vec<(RegionId, Rect)>   (ui)
```

- **Preset** (`ui::layout::default_preset`) turns the visibility flags into a
  tree. This is where every v1 responsive rule lives: the compact-mode header,
  the `two_panel_min_cols` / `three_panel_min_cols` thresholds, the
  automations-pane minimum column height, and the right-column occupant list.
- **Solve** (`ui::layout::solve`) walks the tree, giving each branch's children
  to `ratatui::layout::Layout` and recursing into sub-branches.
- **Project** (`PanelAreas::from_placements`) reads the named regions back out.

## Module ownership

| Type | Module | Why |
|---|---|---|
| `Axis`, `Sizing`, `RegionId`, `Region`, `Node` | `session::workspace_tree` | Pure data with no crate-internal references — `tests/architecture_rules.rs` gives `session` an empty allowlist and this needs nothing, so **no allowlist edge is added** by this change. |
| `default_preset`, `solve`, `PanelAreas` | `ui::layout` | Needs `ratatui::layout::{Layout, Rect}`; `ui` may already reference `session`. |

`session` is the right home even though only `ui` reads the tree today: ADR-V23
persists the tree as `~/.config/thurbox/layout.toml`, and config loaders live in
`agent::*_config`, which may reference `session` but **never** `ui`. Putting the
data anywhere else would force an allowlist change the moment the file ships.
This mirrors `session::AgentDef` / `HostDef` / `theme_config::CustomThemeDef`.

## Key decision: the solver delegates arithmetic to ratatui

`Sizing` has exactly three variants, and each maps 1:1 onto the only three
`Constraint` kinds v1 used:

| `Sizing` | `Constraint` | v1 use |
|---|---|---|
| `Cells(n)` | `Length(n)` | header/footer, the search strip, the status row, the automations pane |
| `Percent(n)` | `Percentage(n)` | the 18/15/20/25/75 column shares |
| `Fill { min }` | `Min(min)` | the content band (`Min(1)`), the center (`Min(0)`), the session list above automations (`Min(3)`) |

So each branch hands ratatui the **byte-identical constraint list** the old code
built by hand. Geometry is unchanged because the arithmetic is literally the same
call, not because it was checked afterwards.

Two consequences worth stating:

- **Zero-extent children are emitted, not omitted.** v1's vertical split always
  passes five constraints, using `Length(0)` for a hidden header / search strip /
  status row, then treats a zero-height rect as absent. The preset does the same
  rather than dropping the child, because a shorter constraint list is a
  different input to the solver and its output is not guaranteed identical. The
  *projection* is what turns a zero-extent band into `None`.
- **The horizontal column list omits hidden occupants**, because v1 omitted
  their constraints too.

## Key decision: `PanelAreas` survives as the projection

`PanelAreas` has 30-odd consumers in `src/app/view.rs`, `src/app/mod.rs` and the
acceptance harness. Keeping it — as a *view* of the placement list rather than
the layout's internal representation — keeps this change to the layout engine
instead of spreading it across the view.

The one field that changes is `plugin_pane: Option<Rect>` →
`plugin_panes: Vec<Rect>`, which drops `Copy` from the derive list (`Clone`
stays). Nothing copies a `PanelAreas`; it is constructed and read.

## Key decision: extra plugin columns are gated by a trial solve

The right column's occupants are 20% each and the center holds `Min(0)`, so when
the shares over-subscribe **the center is the region that starves** — at 120
cols with a session list, an info panel, tasks and the file viewer the center is
already down to single digits, and a fourth 20% column drives it to zero.

The gate is therefore: append plugin leaves, solve, and while the center is
below `CENTER_MIN_COLS` **and more than one plugin leaf is present**, drop the
last plugin leaf and solve again. Bounded by the pane count, and it runs only
when two or more plugin panes are visible.

Scoping it to leaves *past the first* is deliberate: with one plugin pane the
tree is exactly the v1 occupant list, including the narrow-center cases v1
already allowed. A gate that also applied to the first pane would change a
layout that ships today, which the "geometry must not change" constraint forbids.

## Rejected alternatives

- **Write our own flex arithmetic instead of calling `Layout::split`.** The
  honest "real layout engine". Rejected: ratatui's cassowary rounding is exactly
  what the snapshots encode, and re-deriving it would turn a
  behaviour-preserving refactor into a hunt for off-by-ones at every percentage.
  The tree's value is the *structure*; the arithmetic was never the problem.
- **Keep the tree private to `ui`.** Simpler now, wrong later: `layout.toml`
  must be parsed by `agent`, which cannot reference `ui`.
- **Put the solver in `session` too, returning `(x, y, w, h)` tuples.**
  Rejected: it would either duplicate `Rect` or pull ratatui's layout solver into
  the pure-data layer for arithmetic only the renderer needs.
- **Replace `PanelAreas` with a `RegionId → Rect` map at every call site.** The
  end state ADR-V23 implies. Rejected for this change: 30+ consumers would churn
  for no behavioural gain, and named fields catch a typo that a map lookup
  returns `None` for. The map exists — `PanelAreas` is now a projection of it.
- **Keep `PanelAreas: Copy` with a fixed-size array of plugin rects.** Rejected:
  an arbitrary cap is the same mistake as one slot, one order of magnitude up.
- **Add `min_width` / `min_height` node keys and express responsive collapse
  with them.** ADR-V23 lists them as tree keys. Rejected here for two reasons:
  the solver-level rule ("a child under its min is hidden") hides the *starved*
  region, and for the right column that region is the center — which must never
  be hidden, being the fallback view; and with no `layout.toml` nothing can set
  them, so they would be unused schema. The keys land with the file.
- **Emit N plugin leaves with no cap.** Rejected: the agent terminal silently
  disappears at four columns on a 120-col terminal.
- **A `tabs` leaf kind for the central pane's Agent/Shell/Review tabs.** Fits
  the tree, but the tab strip is drawn on the central pane's border by
  `App::draw_central_tabs` and its state is `CentralTab`; converting it is a
  view change with no layout consequence, so it would only add untested surface
  to this change.

## Risks

- **A snapshot moves.** That means the preset does not reproduce v1, and the
  preset is what gets fixed — never the snapshot. The failure is loud and points
  at the exact screen.
- **Per-frame allocation.** The preset allocates a small `Vec` per branch and the
  solve one `Vec` of placements, where v1 allocated ratatui's constraint vectors
  only. Against a demand-driven loop that paints on change (~4 fps idle) this is
  not measurable, and `Layout::split` — the expensive part — is unchanged and
  still hits ratatui's internal layout cache.
