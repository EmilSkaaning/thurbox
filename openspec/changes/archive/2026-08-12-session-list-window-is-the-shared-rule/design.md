# Design — the session list's window becomes the kernel's own rule

## The seam

Four behaviours were derived from `ListState::offset()`, and the refusal enumerated them
separately because a reader who sees only "which rows are on screen" concludes the gap is
a wiring detail. After this change all four are derived from one thing — the window
`ui::visible_item_window` resolved over the pane's own item heights:

| Derived from the offset | Derived from the window |
|---|---|
| which rows are painted | `render_stacked(&children[start..end])`, the painter's own slice |
| `▲ N` / `▼ N` on the border | `start` and `total - end`, reported out of the paint |
| click hitboxes | the rects `render_stacked` returned, one per item |
| the placeholder's index | an index into the same folded items the window counts |

The painter already resolved three of the four for a plugin pane. What it did not do was
say **what it drew**, so a caller that wanted the clipped counts had to recompute the
window — and a second implementation of the window is the thing that made the two panes
disagree in the first place. So the counts come out of the paint, next to the hitboxes,
for the reason the hitboxes come out of the paint: a second walk could resolve against a
layout other than the one on screen.

## Why the pane converges and not the rule

`migration/phase-4` states the constraint precisely: the window "MUST NOT be closed by
redefining the kernel's own windowing rule to match one pane's widget", because that rule
is what every plugin list and three seated panes scroll by. It says nothing about the
pane, and the pane is the thing that is about to be deleted.

The precedent is `migration/handover`'s frame rule: a native pane whose frame differs from
the host's is converged onto the **host's** frame, in its own change, before the handover
— never the other way, and never inside the handover. Convergence in the direction of the
host, ahead of the deletion, is the shape this specification already has for exactly this
problem. A window is the same kind of thing as a frame: a property of how the host draws a
pane, which a handover must not be able to change under cover of moving the drawing code.

### Rejected: teach `visible_item_window` the widget's sticky offset

The obvious symmetric move, and the one the gate names. It fails on two counts. It is
**stateful** — a sticky offset is a value carried between frames — where the view-tree
renderer is a pure function of `(tree, frames, palette)`; threading a per-pane, per-node
offset table through it is a policy change wearing a plumbing change's clothes, and it
would have to be reachable from a plugin pane's paint, which holds no mutable kernel
state. And it changes **four other panes** to fix one, which is the hazard ADR-39 recorded
from the other side.

### Rejected: keep the widget and make the plugin match it

Symmetric in form, impossible in fact: the plugin is never told its height, so it cannot
window anything. The window is the kernel's by construction (ADR-30), and the only
question was which kernel rule.

### Rejected: converge inside the handover

It would make the handover's claim unverifiable. A commit that deletes a renderer *and*
changes how the pane scrolls cannot be reviewed for regressions, because every moved cell
has two candidate causes. Same argument as the frame rule's, and the reason this is a
separate change with its own recorded behaviour note.

## The item shape, and why it is here rather than in the handover

Folding a repo-group header into the row below it is the *identity* half ADR-61 made
expressible, and it has to land with the window because the window counts items: a rule
that windows nine flat children and a widget that windows eight folded items cannot agree
however correct each is. So both trees fold, in the same change that makes both window by
the same rule.

The cost is a moved recording. `tests/snapshots/bundled_session_list__*.snap` are
regenerated, and they are regenerated from the **native** tree — the edge that gives them
provenance is still establishable, because `session_list_tree` still exists. That ordering
is exactly what ADR-48's fourth handover condition is for: a recording taken while the
native builder is present is the one that survives its deletion, and it cannot be produced
afterwards.

## Where the indicators live

`ui::draw_clipped_indicators` goes to `src/ui/mod.rs`, the layer's shared vocabulary,
beside `visible_item_window` — for that helper's reason. It has one caller today and a
second the moment a seated plugin pane needs the same indicators on the frame the kernel
drew around it; leaving it private to a module that is scheduled for deletion would mean
moving it under the handover, where a moved painter is a moved cell nobody can attribute.

It takes a `&mut Buffer` rather than a `&mut Frame`, because the plugin-pane painter has
only the buffer. Same output: two right-aligned `Paragraph`s on the block's top and bottom
border rows, one column in from the right.

## What the pane keeps

The pane keeps everything that needs a **width**, which is the half of ADR-60's split that
was easy to overshoot: `resolve_items` (how much of the agent's reported text fits after
the name, and whether it fits at all), `fit_status_text`, and `pending_spawn_slot`. It also
keeps the placeholder row, which is not a session and has no snapshot yet.

It stops keeping a `ListState`. `App::session_list_state` had exactly one reader and one
writer, both in the paint, so it is deleted rather than left as a field nothing consults —
a retained cursor that no longer decides anything is the quiet kind of stale state the
comment rules exist to keep out.

## Risk, and how it is bounded

The session list is primary navigation, and a regression here makes thurbox unusable. Four
things bound it:

1. **The window rule is already load-bearing** in four panes and has an exhaustive
   reduction test against its pre-ADR-61 form.
2. **The painter is already what draws this pane's rows** — `line_spans` has been the
   native pane's span builder since the port, so no cell inside a row moves. What moves is
   which rows are on screen and where the item boundaries are.
3. **The hitbox test survives unchanged in its claim**: a two-line item is one hitbox
   spanning both lines, which is what the widget did.
4. **The two panes are asserted equal at a size where the window bites**, which is the
   test that replaces the enumerated divergence, and it compares the drawn slice and the
   clipped counts rather than only the trees.
