# Thurbox v2 — Layout

How space is divided between panes, and how a plugin positions content that
does not fit the normal flow.

This supersedes the slot model sketched in
[FEATURES-View-Tree.md §2](FEATURES-View-Tree.md#2-panes-and-roots) and the
flex rules in [§5](FEATURES-View-Tree.md#5-layout-algebra); those sections
remain the plugin-facing summary. Decisions:
[ADR-V22](ARCHITECTURE.md#adr-v22) (anchors),
[ADR-V23](ARCHITECTURE.md#adr-v23) (the workspace tree).

Three of the four layout limitations in
[LIMITATIONS §1](LIMITATIONS.md#1-layout) are resolved here. The fourth is
deliberately not, and §6 says why.

---

## 1. Two layers

```text
┌─ workspace tree ────────────────────────────────────────┐  §2
│  splits and tab groups; leaves are panes                │
│                                                          │
│   ┌─ pane ──────────────┐   base layer   ← normal flow   │  §3
│   │  box / row / column │                                │
│   │   ┌ overlay ┐       │   overlay layer ← anchored     │  §4
│   │   └─────────┘       │                                │
│   └─────────────────────┘                                │
└──────────────────────────────────────────────────────────┘
```

The **base layer** keeps today's invariant: siblings never overlap, one axis
per container, no absolute positioning. The **overlay layer** is where floating
content lives, strictly z-ordered and resolved in a second pass.

---

## 2. The workspace tree

Pane geometry is a **tree of splits**, not a fixed set of slots. A branch is a
horizontal or vertical split; a leaf is a pane or a tab group.

```toml
# ~/.config/thurbox/layout.toml — optional. Absent = the default preset (§2.3).
[workspace]
split = "horizontal"
children = [
  { pane = "sessions", size = "18%", min_width = 20 },
  { split = "vertical", children = [
      { tabs = ["terminal", "review"], weight = 1 },
      { pane = "search", size = 12 },
  ]},
  { pane = "tasks", size = "20%" },
]
```

| Key | Applies to | Meaning |
|---|---|---|
| `split` | branch | `horizontal` \| `vertical` |
| `children` | branch | Ordered child regions |
| `pane` | leaf | A pane id contributed by a plugin |
| `tabs` | leaf | Several panes sharing a region, one visible, tab strip on the border |
| `size` | any | Fixed cells or a percentage of the parent |
| `weight` | any | Share of the remainder, after fixed sizes |
| `min_width` / `min_height` | any | Below this the region is **hidden, not squeezed** |

### 2.1 What this unlocks

Everything [LIMITATIONS §1.2](LIMITATIONS.md#12-pane-geometry) listed falls out of the tree
rather than needing a feature each:

| Wanted | Expressed as |
|---|---|
| A full-width pane above the list and terminal | A vertical root split whose first child is that pane |
| A 2×2 dashboard across panes | A horizontal split of two vertical splits |
| Nested splits | A branch inside a branch |
| Reordering at runtime | Reordering `children` |
| A header-docked pane | A `size = 1` first child of a vertical root |
| Panes stacked in a column | What `left` already was, now explicit |

### 2.2 Slots become placement hints

`slot` does not disappear from the manifest — it becomes the answer to "where
does a **newly installed** pane go when the workspace tree does not name it?"

```toml
[[panes]]
id = "ci"
slot = "right"    # auto-placement hint, not a position
```

The kernel appends the pane to the region its slot maps to. A user who has
never touched `layout.toml` therefore gets the same behavior as today, and a
plugin author still does not have to think about layout.

### 2.3 The default preset

With no `layout.toml`, the kernel synthesizes the tree that reproduces v1's
`PanelAreas` exactly:

```text
vertical
├── header                     (kernel chrome, size 1)
├── horizontal
│   ├── vertical  18%          left:   sessions, automations
│   ├── tabs      weight 1     center: terminal / shell / review
│   └── right     20% each     right:  tasks, files
├── bottom strip               size 12 when shown
├── status band                size 1 when shown
└── footer                     (kernel chrome, size 1)
```

So zero-config output is byte-identical to the slot model, and the slot model
stops being a separate mechanism — it is a preset over the general one.

### 2.4 Responsive collapse

Unchanged from v1 and from the slot model: a region under its `min_width` or
`min_height` is **hidden, not squeezed**, and hiding is a *layout* decision
that does not touch the user's visibility state
([FEATURES-Keybindings §7](FEATURES-Keybindings.md#7-pane-visibility)).
Widening restores it.

### 2.5 What is deferred

**Interactive resize** — dragging a split border with the mouse, tmux-style —
is not in 2.0. The tree makes it possible (a drag writes back a `size`), but it
needs hit regions on split borders, a persistence policy for transient drags,
and keyboard equivalents. It is a follow-up, not a prerequisite, and the file
is editable in the meantime.

---

## 3. Base-layer flow

Unchanged. Inside a pane, layout is flex along one axis per container:

1. Fixed children (`size: n`) take their size.
2. `size: "auto"` children are measured by content.
3. The remainder is distributed among `flex: n` children proportionally.
4. Overflow clips; a `scroll` ancestor makes it scrollable instead.

No absolute positioning, no negative margin, and siblings never overlap. That
constraint is what keeps the solver a single pass for trees that do not use §4.

---

## 4. Anchors — the overlay layer

A node may position itself against **another node's resolved rect** instead of
taking part in the flow:

```lua
ui.box({
    id = "completions",
    anchor = {
        to = "query-input",  -- an id in the same pane
        side = "below",      -- below | above | right | left
        align = "start",     -- start | center | end
        flip = true,         -- use the opposite side when it does not fit
        offset = { 0, 0 },
    },
}, { w.list({ items = matches }) })
```

This is the general capability behind autocomplete dropdowns, context menus,
tooltips, and inline compose boxes — including v1's
`render_compose_inline(frame, diff_area, anchor_y, comp)`, which floats a
comment box at a diff line and flips above or below as room allows. That case
was previously handled by a bespoke `inlineAt` slot on `diff`; **the special
case is removed** in favour of this.

### Rules

1. **Two passes, only when needed.** The base flow resolves first; anchored
   subtrees resolve against the resulting rects. A tree with no `anchor` is a
   single pass, exactly as today.
2. **Clipped to the pane.** An anchor cannot escape its pane's rect in 2.0.
   `flip` and then clamping keep it inside. Escaping to screen bounds needs
   cross-pane z-ordering and is deferred.
3. **Z-order is deterministic**: pane order, then declaration order within the
   pane. There is no `z-index`.
4. **Focus is unchanged.** An anchored subtree belongs to its pane and is not a
   separate focus target, so exactly one pane still holds focus. This is the
   invariant that mattered, and anchors do not touch it.
5. **Hit-testing runs overlay-first**, top of the z-order down, then the base
   layer.
6. **Nesting is capped at 3** (a menu, a submenu, its submenu). Deep enough for
   real UIs, shallow enough that the pass count is bounded.
7. **A dangling `to` is a no-op**, logged once — the subtree is not rendered
   rather than rendered somewhere arbitrary.

### The invariant that changes

The kernel previously assumed nothing ever overlaps. It now assumes:

> The **base layer** never overlaps. The **overlay layer** may overlap the base
> layer and is strictly ordered.

That is the honest restatement, and it is what the monkey test asserts
(§7).

---

## 5. Measurement

`render` stays pure and synchronous — it cannot ask how tall anything is. Two
mechanisms replace the ability, and together they cover the real cases without
making the frame two-way.

### 5.1 Node props for the concrete cases

The kernel owns the renderer, so it knows the mapping a plugin was trying to
reconstruct:

```lua
ui.markdown({ content = content, revealSourceLine = 42 })  -- reveal source line 42
ui.code({ content = content, revealLine = 108 })
ui.scroll({ id = "list", revealNode = "row-17" }, children)
```

This solves "scroll to rendered line N" exactly, rather than approximately.

### 5.2 Opt-in measurement feedback

For the general case, a node may request its resolved rect back:

```lua
ui.box({ id = "card", measure = true }, children)
```

The kernel delivers, with the next event batch:

```lua
{ type = "measure", rects = { card = { x = x, y = y, width = w, height = h } } }
```

It is **one frame late** by construction. That is fine for virtualization
windows, follow-the-selection scrolling, and progressive layouts; it is not
fine for anything that must be correct in the frame it is computed.

### 5.3 Still impossible

Single-pass content-driven layout: sizing a box to its wrapped content *and*
positioning a sibling relative to that result within the same frame. A plugin
can converge over two frames; it cannot do it in one. Masonry is therefore
approximate, and this stays a documented limit.

---

## 6. Cross-pane alignment stays unsupported

Two sibling panes cannot share a column ruler, and this is **not** being fixed.

The mechanism would be named size groups — a node declares `sizeGroup: "cols"`
and the kernel resolves every member to a common width across panes. It is well
understood and it would work.

It is rejected on value, not difficulty. It requires a solver pass that couples
panes which are otherwise independent, it interacts badly with a pane being
hidden by §2.4 (does the group re-solve, or keep a ruler for content nobody can
see?), and the demand is one hypothetical: a table spanning two panes, which in
a terminal is almost always better as one pane. Adding cross-pane coupling to
the solver for that is a bad trade.

---

## 7. Testing

| Property | How |
|---|---|
| The default preset reproduces v1 | The existing insta snapshots, unchanged, against the synthesized tree |
| Base layer never overlaps | Monkey invariant over random trees |
| Overlay z-order is deterministic | Same tree renders identically across runs |
| Anchors stay inside their pane | Property test: random anchor + random pane rect, assert containment |
| `flip` picks the side with room | Table-driven test at each edge |
| Single-pass when no anchors | A perf counter asserting pass count is 1 for anchor-free trees |
| Collapse is layout-only | Narrowing then widening restores the pane without touching visibility state |
| Measurement is one frame late | Fixture asserting the rect arrives in the following batch, not the current one |

---

## 8. Cost

Stated plainly, because this is the largest single addition to the v2 kernel:

- A real layout engine replaces `compute_layout`'s 10 fixed rects with a
  recursive solver. Arguably simpler than what it replaces, but it is new code
  on the frame path.
- Two-pass layout and an overlay buffer, paid only by trees that use anchors.
- `layout.toml` becomes a config file with a schema, validation, and a
  migration story.
- The hit-test registry gains z-ordering.

What it buys: five of the six items in
[LIMITATIONS §1.2](LIMITATIONS.md#12-pane-geometry), the whole of
[§1.1](LIMITATIONS.md#11-floating-elements), most of
[§1.3](LIMITATIONS.md#13-measurement), and the deletion of the
`diff.inlineAt` special case.
