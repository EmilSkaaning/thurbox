# Design

## 1. Where the rule lives, and why it is one function

`ui::visible_window` is the layer's shared windowing rule and has four callers: the
plugin-pane renderer, the selector-modal rows, the automation run history and the theme
picker's own variant. Three of them window uniform single-line rows and will never want
anything else.

So the generalisation is **one** function with a height accessor, and the old signature
is a wrapper:

```rust
pub(crate) fn visible_window(total: usize, selected: usize, height: usize) -> (usize, usize) {
    visible_item_window(total, |_| 1, selected, height)
}

pub(crate) fn visible_item_window(
    total: usize,
    item_rows: impl Fn(usize) -> usize,
    selected: usize,
    height: usize,
) -> (usize, usize)
```

Ownership: `src/ui/mod.rs`, where `visible_window` already is. No new module, no new
architecture edge, `tests/architecture_rules.rs` untouched — the rule is pure arithmetic
over a closure and reaches nothing.

**An accessor rather than a `&[u16]`** because the three uniform callers must not have to
allocate a vector of ones to keep their behaviour, and because the plugin renderer wants
to measure a child's height with the same `height_of` walk that will draw it — passing a
closure is what lets the rule ask only about the items it actually walks.

## 2. The reduction is the safety property, so it is stated as arithmetic

Every existing list has one-line children, so the whole risk of this change is that the
general rule disagrees with the old one somewhere. The generalisation is therefore
written step for step against the original, each step reducing when `item_rows ≡ 1`:

| Old (items) | New (rows) | Reduces because |
|---|---|---|
| `total <= height` → `(0, total)` | `Σ rows < = height` → `(0, total)` | `Σ 1 = total` |
| `margin = (height/4).min(3)` | unchanged | the margin is a count of **items**, and it always was: it is "how far above the cursor the window starts" |
| `start = selected - margin` | unchanged | — |
| `start.min(total - height)` | `start.min(last_start)`, where `last_start` is the largest index whose remaining items still fill `height` rows | with unit heights the tail that fills `height` rows is exactly `height` items, so `last_start = total - height` |
| `end = (start + height).min(total)` | `end` = as many items from `start` as fit in `height` rows | with unit heights, `height` of them |

Two clauses exist only for the case unit heights cannot produce, and each is proved
unreachable for them rather than merely believed to be:

- **A list may draw at least one item even if it does not fit.** An item taller than the
  pane would otherwise produce an empty slice. With unit heights and `height >= 1` the
  first item always fits, so the clause never fires.
- **A tall neighbour above the cursor may push the cursor off the bottom.** With unit
  heights `end - start = height > margin >= selected - start`, so the cursor is always
  inside the slice and the correction never fires.

`the_general_rule_reduces_to_the_uniform_one` walks every
`(total ≤ 24, selected < total, height ≤ 24)` triple and requires the two forms to return
the identical pair — 3,600-odd cases, which is the whole domain the three uniform callers
can reach at any realistic pane size. A proof by exhaustion is worth more here than an
argument, because the argument above is exactly what a future edit would break silently.

## 3. Rejected: teaching `visible_window` the widget's sticky offset

The obvious way to make the two panes agree is to give the shared rule ratatui's policy —
hold the offset until the cursor leaves the viewport — since that is what the pane being
handed over does.

Rejected, for the reason `tests/session_list_pane_handover_gap.rs` already records from
the other side: that helper is what every plugin list and three native panes scroll by, so
a change for this pane changes all of them. Worse, the widget's rule is **stateful** — it
takes the previous offset as an input — and the view-tree renderer is deliberately a pure
function of `(tree, frame table, palette)` with no state and no path back to a VM. Making
it sticky means a per-pane, per-node offset table threaded through the renderer the way
`FrameTable` is, resolved and written back by the kernel. That is a real design with real
precedent (`App::motion`), and it is a *scrolling policy* change wearing a *plumbing*
change's clothes: it would move which rows are on screen in four panes, in the same commit
that made a list's rows measurable. Two changes.

This change is deliberately shaped so that decision stays open and un-prejudged. Nothing
here makes the sticky rule harder to add later: it would be a second implementation of the
same signature, chosen by a declaration on the list node.

## 4. Rejected: a new `item` node kind

`ui.list({ ui.item({header, row}), … })` reads well, and it is what ratatui's own
`ListItem` is.

Rejected because a `column` inside a `list` is already exactly that node, already gets one
rect from `render_stacked`, already gets one hitbox from the row sink, and already counts
as one index for the cursor. The only thing it did not do was scroll correctly, which is
this change. Adding `item` would put a second spelling of an existing container into a
catalog whose stated discipline is that it is "the set thurbox's own panes need, not a
general drawing API" — and every walk over the tree (`children`, `depth`, `node_count`,
`is_inlineable`, `height_of`, conversion, the recorder) would grow an arm to say "same as
a column".

The cost of the rejection is that the grouping is a *convention* rather than a type: a
plugin that wants one row per record must remember to wrap. That is a documentation
problem — the declared type surface says so — and not a correctness one, because a plugin
that does not wrap gets exactly today's behaviour.

## 5. Rejected: publishing the rows already grouped

The kernel could publish the session list's rows with their headers already folded in, so
the plugin's array and the tree's children are 1:1 by construction and no windowing
question arises.

Rejected on the rule ADR-29 set and every port since has applied: the kernel publishes a
*rendering* only when two panes must agree about it. A group header is one pane's
presentation of a fact (`row.group`) the snapshot already carries, and folding it into the
publication would make the plugin an arranger of strings the kernel composed — the same
objection that stopped ADR-27 from publishing `"8.0/16.0 GB"`. It also would not help any
*other* pane with a multi-line row.

## 6. The scroll track measures the same quantity as the window

`ui::scrollbar::reserve_track(area, content_len, viewport)` decides whether a list
overflows, and `draw_into(.., content_len, viewport, position)` sizes and places the thumb.
Both are handed the child **count** today, against a viewport in **rows** — a comparison
that is only meaningful because the two have been equal.

They become row quantities: `content_len` is the list's total rendered rows, `position` is
the number of rows above the selected item. For a list of one-line children every value is
what it was, so the file viewer's track — the only declared one in the tree — is
byte-identical. For a list of taller items the track now describes what the user is
actually scrolling through.

The alternative (leave the track in item space, window in row space) was rejected because
a thumb whose length says "you can see 10 of 20" while the pane shows 5 is worse than no
thumb: it is a wrong answer rather than a missing one.

## 7. What this costs per frame

The window is only resolved for a list that declares a cursor or a track, and the heights
are only measured when one of those holds. Every other list — the info panel's gauge
column, every `ui.list(rows)` with no cursor — takes the same path it took before and
measures nothing.

For a list that does scroll, the change is one `height_of` call per child, including the
children off screen. `height_of` is O(1) for the text, line, fill and divider nodes every
scrolling list in the tree is built from; it is only expensive for a `paragraph`, whose
wrap it measures. The walk is bounded by `MAX_NODES` (4096) like everything else, and the
demand-driven render loop is untouched: nothing here paints more often, and a plugin pane's
tree is still built off the UI thread.

Measured rather than asserted would be better, and is not offered: there is no
`perf_counter` for a windowing walk and inventing one for a bounded O(n) pass over nodes
already in cache would be more machinery than the fact deserves.

## 8. What the gate says afterwards

`the-window-is-the-list-widgets` stays **blocked**, and its probe changes in one half:
`plugin_window_is_the_shared_rule` reads `super::visible_item_window` instead of
`super::visible_window`, because that is the helper the renderer now calls. The row's
`stands` text is rewritten to record what moved — the plugin's window is now over items of
declared height rather than over flat single rows, so the two panes no longer disagree
about *what a row is*, only about *which rows sit beside the cursor*.

This is the tightening the sibling gates have had to do three times before (a write-shaped
binding, a node named `Fill`, a `cfg!` that answered about the test binary): a probe whose
needle stops matching for a good reason is updated, and the verdict is not flipped.
