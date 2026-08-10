# Design — the inline line node

## 1. Where the pieces live

```text
session/view_tree.rs   ViewNode::Line(Vec<ViewNode>) + the inlineable predicate
   (pure data)         — the rule about which kinds may appear in a line
        │
plugin/view.rs         Lua table ─▶ ViewNode::Line, refusing a child with no
        │              intrinsic width (ViewError::NotInlineable)
        │
ui/plugin_pane.rs      intrinsic-width measurement and the flatten-to-spans
                       paint; the only place a terminal column is counted
```

The split follows the existing one exactly. The *rule* (which kinds are
inlineable) is a property of the node catalog, so it lives with the catalog in
`session` where both the converter and the renderer can read it. The
*measurement* (how many terminal columns a run occupies) is a rendering
concern and stays in `ui`, which already depends on `unicode-width`.

Putting the measurement in `session` would have been the mistake: conversion
would then bake a column count into the tree, and that count is only correct for
the terminal that happens to be attached.

## 2. Why the restriction is a predicate on the node, not a flag threaded
through conversion

A line may hold a `motion`, and a motion's frames are converted by the ordinary
`convert` walk — which knows nothing about the line above it. Two ways to stop a
`column` reaching a line through a motion frame:

1. Thread an `inline: bool` down `convert`, so every frame conversion knows it
   is inside a line.
2. Convert normally, then ask the finished child whether it is inlineable.

The second is chosen. It is one recursive predicate on `ViewNode`
(`Text` → yes, `Line` → all children, `Motion` → all frames, everything else →
no) rather than a parameter on four functions, and it cannot be forgotten at a
new call site: a future node kind defaults to *not* inlineable, which is the
safe direction. The cost is walking a converted subtree once more, bounded by
`MAX_NODES` which conversion already enforced.

## 3. Why a motion reserves its widest frame

A motion inside a line is the point of the feature — an animated status glyph
next to a name is the shape v1's session list has. If the line laid each frame
out at that frame's own width, every run to its right would move whenever the
frame width changed. For a braille spinner (all frames one column) nothing
moves; for `…`/`. .`/`. . .` the whole row would jitter, and a user reads that
as the pane redrawing wrongly, not as an animation.

So a motion in a line occupies `max(width(frame))` for its whole life, and a
narrower frame is padded on the right. This is the same rule and the same
rationale as `height_of`'s existing "a motion takes the tallest of its frames",
which already exists to stop siblings shifting vertically.

Padding is on the right rather than centred: a leading glyph is the common case,
and left-aligned padding keeps the *start* of the animation fixed, which is what
the eye tracks.

## 4. Why clipping rather than wrapping

`text` does not wrap — a plugin splits lines by returning separate nodes — and
`height_of` therefore takes no width parameter. A wrapping line would be the
first node whose height depends on width, which would either force a width
parameter through the whole height pass or leave the layout under-counting rows
and overdrawing its siblings. Neither belongs in the same change as the node
itself, so an over-long line is clipped at the pane edge like every other text
node in the tree.

## 5. Alternatives rejected

**Width hints on `row`** (`size = 3`, `flex = 1`). This is what a general layout
engine would do, and it is the reason not to: the kernel then owns
over-subscription policy, truncation priority, and what a percentage is relative
to, forever, for every node kind. The spike's blocker does not need any of that
— it needs runs at their natural width — and `row`'s equal shares have a stated
rationale worth keeping.

**A `spans` field on `text`.** Fewer node kinds, but it makes `text` two things
(a run and a container), and a motion could not sit among the spans without
`text` gaining children — which is precisely the shape a container node already
is. A line is a container; the catalog already has the concept.

**Reusing `row` with an intrinsic-width mode flag.** One node, two layout
behaviours selected by a boolean: a plugin author would have to know which mode
a given pane wants, and every existing `row` in a plugin would keep working
while meaning something subtly different from the `row` next to it. Two names
for two behaviours is cheaper to learn.

## 6. What this does not resolve about Phase 4

The spike listed three conditions; this closes only the first. Kernel-owned
selection and an event-driven render trigger are separate, and no host binding
yet exists through which a plugin can read the session list — that gap is
unchanged by this change and remains the next thing in the way of a real
bundled pane.
