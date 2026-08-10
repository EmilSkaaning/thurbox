## ADDED Requirements

### Requirement: Pane geometry is a tree of splits

The layout SHALL divide the drawing area with a **tree** whose branches are
single-axis splits carrying their children in order, and whose leaves each name
exactly one region. Every rect the frame paints MUST come from solving that
tree; no region may be positioned by arithmetic outside it.

A branch MUST distribute its extent along one axis only, and sibling regions
MUST NOT overlap.

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

### Requirement: A child declares how it takes its share

Each child of a branch SHALL declare its size as one of: a fixed number of
cells, a percentage of the parent, or a fill share taking the remainder with a
stated floor. A branch MUST honour those declarations without the caller
computing offsets.

#### Scenario: A fixed child keeps its size when a sibling grows

- **WHEN** a branch holds a fixed-cell child and a fill child, and the branch's
  extent grows
- **THEN** the fixed child's extent is unchanged and the fill child absorbs the
  difference

#### Scenario: A zero-cell child occupies no space

- **WHEN** a child declares zero cells
- **THEN** it resolves to a zero-extent rect and its siblings are positioned as
  if it were absent

### Requirement: The default preset reproduces the v1 panel layout exactly

With no user-supplied layout, the kernel SHALL synthesize the tree that
reproduces the previous fixed-rect layout **exactly**: the same header/footer
bands, the same 2-panel and 3-panel width thresholds, the same left-column
sessions/automations split, and the same right-column occupant order and
shares. For every terminal size and every combination of panels the previous
implementation could show, the solved rects MUST equal what it produced.

#### Scenario: Pinned screens do not move

- **WHEN** the acceptance snapshots are rendered from the solved tree
- **THEN** every snapshot matches its recorded content byte for byte

#### Scenario: Width thresholds are unchanged

- **WHEN** the terminal is at each panel-visibility threshold (below the
  two-panel minimum, between the two thresholds, and at or above the
  three-panel minimum)
- **THEN** the same panels are placed as before the tree was introduced

#### Scenario: Every layout assertion holds unmodified

- **WHEN** the layout unit tests written against the fixed-rect implementation
  run against the solved tree
- **THEN** they pass without their expectations being changed

### Requirement: The right column seats every visible plugin pane

The right column SHALL hold **one region per visible plugin pane**, after the
tasks panel and the file viewer, in the order the panes were published. The
number of plugin panes the layout can place MUST NOT be fixed at one.

#### Scenario: Two visible panes get two regions

- **WHEN** two plugin panes are visible in a terminal wide enough for both
- **THEN** the layout reports two plugin regions, adjacent along the row, that
  do not overlap each other or any native panel

#### Scenario: A hidden pane leaves no gap

- **WHEN** one of several visible plugin panes is hidden
- **THEN** the remaining panes keep their sizes and the freed width goes to the
  center region, leaving no gap where the hidden pane was

#### Scenario: No plugin pane changes nothing

- **WHEN** no plugin pane is visible
- **THEN** the layout reports no plugin region and is identical to a build
  compiled without the plugin host

### Requirement: An extra plugin column never squeezes the center away

Plugin regions past the **first** SHALL be placed only while the center region
would keep at least a minimum width. A plugin column that does not fit MUST be
**hidden rather than squeezed**, and MUST be placed again when the terminal is
wide enough. The first plugin region MUST follow the native side panels' rule
unchanged, so no previously reachable layout is altered.

#### Scenario: A narrow terminal drops the extra columns

- **WHEN** more plugin panes are visible than the terminal has room for
- **THEN** the leading panes are placed and the trailing ones are not
- **AND** whenever more than one plugin column is placed, the center region is
  at least the minimum width

#### Scenario: Widening restores a dropped column

- **WHEN** a terminal that dropped a plugin column is widened enough to fit it
- **THEN** that column is placed again without the pane's visibility state
  having changed

#### Scenario: One pane is placed exactly as before

- **WHEN** exactly one plugin pane is visible
- **THEN** it is placed wherever the previous single-slot implementation placed
  it, including in terminals too narrow to satisfy the minimum center width
