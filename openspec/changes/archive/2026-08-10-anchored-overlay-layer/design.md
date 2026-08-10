# Design — the anchored overlay layer

## The shape

Two pieces, split along the line the architecture rules already draw:

| Type | Module | Why there |
|---|---|---|
| `Side`, `Align`, `CrossExtent`, `Overlay`, `Overlay::place` | `src/session/overlay.rs` | Pure data plus pure geometry. References no crate module, exactly like `session::workspace_tree`. An anchor is destined to be declarable in config, and config loaders live in `agent`, which may reference `session` but never `ui`. |
| `OverlayLayer` | `src/ui/overlay.rs` | Holds the *per-frame* ordered list and reports hit order. That is view state, and `ui` is where per-frame registries already live (`click_targets`, `scrollbar_hits`). |

Checked against `tests/architecture_rules.rs`: its allowlist is keyed on
*top-level* modules, so neither file needs an entry — `session::overlay`
references no crate module at all (`session`'s allowance is empty, and
`every_module_is_governed` already covers the directory) and `ui::overlay`
references `session`, which `ui` is already allowed. **No allowlist edge is
added**, so neither `CLAUDE.md`'s dependency block nor `docs/CONSTITUTION.md` §2
changes on that account.

`session::overlay` uses `ratatui::layout::Rect`. That is not a new dependency
direction: `session::theme_config` already imports `ratatui::style::Color`, and
ratatui is an external crate, not a crate module. A rect is the only vocabulary
in which "place this against that" can be stated, and duplicating it in
`session` to avoid the import would buy nothing.

## The resolution rule

`Overlay::place(&self, target: Option<Rect>, clip: Rect) -> Rect`, in one pass:

1. Clamp the declared extents to the clip. This is what makes containment
   unconditional instead of a special case.
2. Compute the preferred origin on `side` from the target's corresponding edge.
3. If the overlay fits inside the clip there, take it.
4. Else, if `flip`, compute the opposite side's origin and take it if it fits.
5. Else, dock flush against the clip's edge in `side`'s direction.

An absent target skips straight to step 5 — which is the honest reading of "the
line you are commenting on scrolled out of view": there is no rect to sit
against, so sit as far in the requested direction as the pane allows.

The cross axis is independent: a `Stretch { inset }` spans the clip inset on
both sides (floored at one cell); a `Cells(n)` is aligned against the *target*
per `Align` and then clamped into the clip.

## Why this reproduces `render_compose_inline` by construction

v1's eleven lines are three branches, and each is a step above:

| v1 | Resolver |
|---|---|
| `ay + 1 + h <= bottom` → `top = ay + 1` | step 3, `side = Below` (the target row is one row tall, so its bottom edge *is* `ay + 1`) |
| `ay >= area.y + h` → `top = ay - h` | step 4, flipped to `Above` (bottom edge = target top edge) |
| `_` → `top = bottom - h` | step 5, dock to the clip's bottom |
| `x = area.x + 1`, `w = max(area.width - 2, 1)` | `CrossExtent::Stretch { inset: 1 }` |
| `h = area.height.clamp(3, 6)` | stays in the caller — it is the compose box's own size policy, not an anchor rule |

The correspondence is exact, which is why the port needs no new expectation in
any existing test.

**The one behavioural divergence, stated plainly.** v1 computed
`h = area.height.clamp(3, 6)`, so in a diff area of one or two rows it built a
three-row box and docked it at `bottom - h` — *above* the pane's top edge,
painting over whatever pane sat above. Its `x = area.x + 1` did the same thing
across, escaping a pane one column wide. Step 1 clamps both extents to the clip
instead, so the box shrinks and stays inside. This is reachable only while
composing in a diff area under three rows tall or one column wide; no snapshot
covers it, and the ADR's containment rule requires it. It is a fix, not a
regression, and it is the only case where the ported geometry differs —
`compose_anchor_reproduces_the_legacy_inline_placement` sweeps every anchor row
across heights 3..23 and widths 2..60 against the old formula kept as an oracle,
and `compose_anchor_clamps_a_pane_too_short_for_the_box` pins the divergence.

## Hit-test priority

`CodeReviewHits` gains `overlay: Vec<Rect>` (topmost first). `App::view` records
them as `ClickAction::OverlayCapture` **before** the diff rows, reusing the
click registry's existing first-match-wins ordering — the same mechanism that
already makes a row target win over its pane's whole-rect focus fallback. So
"overlay-first hit-testing" needs no new dispatch path, only a recording order
and a variant that consumes the click.

That closes a real v1 bug: clicking inside the compose box ran
`cr_click_row`, moving the diff selection while the box went on commenting on
the line it was opened for.

## Rejected alternatives

- **Keep `render_compose_inline`, add the layer alongside it.** Two
  implementations of the same rule, one of them the shipping one. The migration
  plan's whole point is that the special case is retired *before* anything
  depends on it; leaving it means the pane gets migrated twice.
- **`diff.inlineAt` — a slot on the diff node.** The design this replaces. It
  blesses the one case v1 had already built and leaves every other pane with no
  route to a floating element, so the second consumer re-opens the question.
- **A `z-index` property.** Familiar and unbounded. Declaration order is enough
  for menus, dropdowns and compose boxes, and cannot be abused into a layering
  war between plugins.
- **`anchor.to = "<node-id>"` now.** There is no id space to resolve against on
  this branch: a pane's contents are Rust render functions. The lookup, and the
  "a dangling `to` is a no-op, logged once" rule that goes with it, land with the
  plugin node tree. Shipping the policy without the lookup would be an
  unreachable branch.
- **A `MissingTarget { Hide, Dock }` policy.** Same reason: `Hide` exists for a
  dangling id, and nothing can dangle here. One rule — dock — covers the one
  reachable case (target scrolled out of view) and matches v1.
- **An `offset: (i16, i16)` nudge.** ADR-V22 lists it, and no surface wants one.
  It would need three more specified interactions (does the fit test use the
  shifted rect? does the flip? does the dock?) to serve a hypothetical gap. It
  lands with the first overlay that needs to not touch its target.
- **A nesting cap of three.** Nesting means anchoring to an anchored subtree,
  which again needs node ids. A cap on something that cannot happen is
  unenforceable and untestable.
- **Intersecting the resolved rect with the clip afterwards.** Equivalent
  containment, worse result: a six-row box in a two-row pane would render its
  border and lose its content, whereas clamping the extent first produces a
  two-row box that renders coherently.
- **Escaping the pane rect.** Wanted for a dropdown at a narrow pane's edge, and
  it needs cross-pane z-ordering plus an answer for the owning pane being hidden
  mid-interaction. Deferred, as ADR-V22 already says.
- **A perf counter asserting the pass count.** The structural fact is stronger
  and cheaper to check: a pane with nothing anchored reports an empty overlay
  list, so there is no second pass to count. A counter would add app-layer
  surface to restate that.

## Testing

- `session::overlay` unit tests: the four sides, flip taken and refused, dock on
  both the no-room and no-target paths, containment over a target swept across
  and past the clip, extent clamping, stretch inset, the three alignments, and
  alignment clamped at the clip edge.
- `ui::overlay` unit tests: declaration order reported topmost-first, empty
  layer for a pane with nothing anchored.
- `ui::code_review` unit tests: the compose rect equals the pre-port rect in all
  three branches, asserted against literal rects derived from the v1 formula.
- Unchanged: every acceptance snapshot and every layout test. The port must move
  none of them.
