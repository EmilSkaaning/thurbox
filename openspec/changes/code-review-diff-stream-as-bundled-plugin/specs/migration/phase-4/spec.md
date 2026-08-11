# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A pane may be ported in part when its whole is not expressible

When a native pane is too large or too geometry-dependent to reproduce whole, the
port SHALL reproduce a named **core** of it completely, and SHALL itemise
everything left out in its proposal — one entry per omitted behaviour, each with
the reason it could not be drawn.

A partial port MUST NOT approximate what it omits. Drawing a diff without its row
tint, or a header without its rule, would make the reproduction agree with nothing
and the record a claim about a pane that does not exist.

The chosen core is the code-review view's **unified diff stream's lines**: the
line-number gutter, the syntax-coloured body, the insertion and deletion row
tints, and the cursor's row.

#### Scenario: The core is reproduced completely

- **WHEN** the native renderer and the plugin are given the same diff line
- **THEN** the two paint the same row, across additions, deletions, context, the
  cursor's row, an empty body, and each colour the highlighter assigns

#### Scenario: The remainder is a list, not a gap

- **WHEN** the port's proposal is read
- **THEN** every unported behaviour of the pane is named with the reason it is
  unported, and the readiness document carries the same list

### Requirement: A reproduction whose native pane is not refactored is validated by frame equality

When a port does **not** refactor the native pane to draw the view tree it is
compared against, the tree builder SHALL be pinned to the **untouched** native
renderer by painting both and requiring the resulting frames to be identical.

Comparing a plugin only against a tree builder written in the same change is
insufficient: two functions agreeing about a format neither is obliged to match is
not evidence. The frame comparison is what closes the chain onto what the pane
paints today.

The reason the native pane was not refactored MUST be recorded, and it MUST be a
property of the pane rather than a preference — for this pane, that its painter
windows a body by character count against a resolved width, which no geometry-free
tree can express.

#### Scenario: The tree builder is pinned to the renderer

- **WHEN** the geometry-free tree and the native renderer's row are each painted at
  the same width
- **THEN** the two buffers are identical cell for cell

#### Scenario: The native pane is unchanged

- **WHEN** the port's diff is inspected
- **THEN** the native paint path is untouched and no pinned frame moves

### Requirement: The view tree's node budget is a whole-tree bound a per-row pane cannot respect

The migration record SHALL state that the view tree's node budget is a bound on a
whole converted tree, while a pane's cost is per row — so a pane whose rows have
unbounded internal structure cannot keep inside the budget by publishing fewer
rows alone.

The measurement MUST be recorded with the pane that produced it: how many nodes a
representative row of this pane costs, how many rows the budget therefore permits,
and that a pathological row can exceed the budget at any row cap.

The consequence MUST be recorded too: the budget is spent building rows the kernel
then windows away, because the plugin builds every row it publishes and the kernel
chooses the visible slice afterwards.

#### Scenario: The budget refuses a real diff

- **WHEN** a plugin returns one row per line of a diff of a few hundred lines with
  syntax-coloured bodies
- **THEN** conversion refuses the tree for exceeding the node budget, and the pane
  reports that rather than drawing a shorter diff

#### Scenario: The bound is recorded, not silently absorbed

- **WHEN** the readiness document is read
- **THEN** it names the node cost of a row, the row cap that cost forces, and the
  two ways out with the reason neither is designed yet
