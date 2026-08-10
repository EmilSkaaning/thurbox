# layout/workspace-tree Specification

## MODIFIED Requirements

### Requirement: Pane geometry is a tree of splits

The layout SHALL divide the drawing area with a **tree** whose branches are
single-axis splits carrying their children in order, and whose leaves each name
exactly one region. Every rect the frame paints MUST come from solving that
tree; no region may be positioned by arithmetic outside it.

A branch MUST distribute its extent along one axis only, and sibling regions in
the **base layer** MUST NOT overlap.

Overlapping is confined to the **overlay layer** (`layout/overlay`): a rect
anchored against another rect, resolved after the base tree is solved, may cover
base-layer content, is clipped to the pane that owns it, and is strictly ordered
by declaration. No base-layer region may overlap anything.

#### Scenario: A nested split resolves to disjoint rects

- **WHEN** a vertical branch contains a horizontal branch that contains two
  leaves
- **THEN** both leaves receive rects inside their parent branch's rect, side by
  side along the horizontal axis, and they do not overlap

#### Scenario: Every placed region is named once

- **WHEN** the tree for any panel-visibility combination is solved
- **THEN** each region id appears at most once in the result, so no two rects
  can claim the same region

#### Scenario: Solving is a pure function of the tree and the area

- **WHEN** the same tree is solved twice against the same area
- **THEN** the resulting rects are equal

#### Scenario: An overlay does not disturb the base layer

- **WHEN** overlays are placed inside a solved tree's regions
- **THEN** every base-layer region keeps the rect the solve gave it, and no two
  base-layer regions overlap
