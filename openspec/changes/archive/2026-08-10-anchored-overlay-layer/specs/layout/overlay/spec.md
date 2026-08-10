# layout/overlay Specification

## ADDED Requirements

### Requirement: An overlay is positioned against a target rect, not by the flow

A node MAY declare that it sits against **another rect** — its *target* —
instead of taking a share of a split. The kernel SHALL resolve such a node in a
**second pass**, after the base tree is solved, and place it flush against the
target on a declared side: below, above, left, or right.

Resolving an overlay MUST NOT change any base-layer rect. The base tree's
solution MUST be identical whether or not overlays are placed against it.

#### Scenario: An overlay sits flush against the requested side

- **WHEN** an overlay with room to spare declares side *below* against a target
- **THEN** its top edge is the target's bottom edge, and its bottom edge is that
  plus the overlay's declared extent

#### Scenario: Each of the four sides places the overlay adjacent to the target

- **WHEN** the same overlay and target are resolved once per side
- **THEN** *below*/*above* place it under/over the target on the vertical axis
  and *right*/*left* place it after/before the target on the horizontal axis,
  each touching the target's corresponding edge

#### Scenario: The base layer is unaffected

- **WHEN** a workspace tree is solved, and then an overlay is placed inside one
  of its regions
- **THEN** every region's rect is unchanged

### Requirement: An overlay flips to the opposite side when the requested one has no room

An overlay MAY declare that it accepts the **opposite** side. When the requested
side cannot hold the overlay's full extent inside the clip and flipping is
accepted, the kernel SHALL place it on the opposite side if the full extent fits
there. When the requested side does have room, the overlay MUST NOT be flipped,
and when flipping is not accepted it MUST NOT be flipped regardless of room.

#### Scenario: No room below places the overlay above

- **WHEN** a flipping overlay declares *below* against a target so close to the
  clip's bottom edge that the overlay would not fit, and the space above the
  target is large enough
- **THEN** the overlay's bottom edge is the target's top edge

#### Scenario: Room on the requested side wins

- **WHEN** a flipping overlay declares *below* and the space below the target is
  large enough
- **THEN** it is placed below, even though the space above is also large enough

#### Scenario: A non-flipping overlay stays on its side

- **WHEN** an overlay that does not accept flipping declares *below* with no room
  below and ample room above
- **THEN** it is not placed above

### Requirement: An overlay with nowhere to sit docks to the clip's far edge

When neither the requested side nor the flipped side can hold the overlay, the
kernel SHALL place it flush against the clip's edge **in the requested
direction** — the bottom edge for *below*, the top for *above*, the right for
*right*, the left for *left*. The same placement SHALL be used when the target
is **absent**, which is how an overlay behaves when the thing it points at has
scrolled out of view.

An overlay MUST always resolve to some rect: it is never dropped for lack of
room.

#### Scenario: A clip too short for either side docks the overlay

- **WHEN** an overlay declares *below* against a target with too little room
  below and too little above
- **THEN** its bottom edge is the clip's bottom edge

#### Scenario: An absent target docks the overlay

- **WHEN** an overlay declares *below* and no target rect is supplied
- **THEN** it is placed exactly where a target with no room on either side would
  have put it — flush against the clip's bottom edge

### Requirement: An overlay never escapes its pane

The resolved rect SHALL be **contained in the clip** on both axes, for every
target position and every declared extent. An overlay whose declared extent
exceeds the clip MUST be **shrunk to the clip** rather than allowed to overflow
into a neighbouring pane.

#### Scenario: Containment holds for any target and clip

- **WHEN** an overlay is resolved against a target placed at each position
  inside a clip, including flush against every edge and outside it
- **THEN** the resolved rect's edges are all inside the clip's edges

#### Scenario: A clip smaller than the overlay shrinks it

- **WHEN** an overlay declaring more rows than the clip has is resolved
- **THEN** its height equals the clip's height and its rect is still contained

### Requirement: An overlay declares its cross-axis extent and how it aligns

An overlay SHALL declare its extent **across** the anchored axis as either a
fixed number of cells or a **stretch** spanning the clip inset by a stated
number of cells on each side. A fixed cross extent SHALL be aligned against the
**target** — at its start, centred on it, or at its end — and then clamped into
the clip.

#### Scenario: A stretch spans the clip

- **WHEN** an overlay declares a stretch inset by one cell inside a clip
- **THEN** its left edge is one cell inside the clip's left edge and its width
  is the clip's width less two, never less than one cell

#### Scenario: Each alignment anchors to a different target edge

- **WHEN** an overlay with a fixed cross extent narrower than its target is
  resolved once per alignment
- **THEN** *start* shares the target's left edge, *end* shares its right edge,
  and *centre* is equidistant from both within a cell

#### Scenario: Alignment is clamped into the clip

- **WHEN** an aligned overlay would extend past the clip's right edge
- **THEN** its right edge is the clip's right edge and its width is unchanged

### Requirement: Overlays are ordered by declaration and hit-tested first

Overlays within a pane SHALL be ordered by **declaration order** — later
declared is drawn on top — with no numeric depth property. The pane MUST report
its overlay rects **topmost first** so that click hit-testing consults them
**before** the base layer, and a click landing on an overlay MUST NOT reach the
base-layer target underneath it.

An overlay MUST NOT be a focus target: it belongs to its pane, and exactly one
pane holds focus whether or not overlays are showing.

#### Scenario: The later declaration is on top

- **WHEN** two overlays are declared in a pane
- **THEN** the reported order is the second, then the first

#### Scenario: A click on an overlay does not reach the base row beneath it

- **WHEN** the compose box is open over a diff row and that row's cells are
  clicked
- **THEN** the diff selection does not move

#### Scenario: No overlay means no overlay pass

- **WHEN** a pane renders with nothing anchored
- **THEN** it reports no overlay rects and hit-testing consults only the base
  layer

### Requirement: The code-review compose box is placed by the overlay layer

The code-review comment compose box SHALL be positioned by declaring an anchor
against the **selected diff row's rect**, resolved by the shared overlay layer.
Its bespoke placement arithmetic MUST be removed from the review renderer, and
the placement it produced MUST be reproduced exactly: one row below the selected
line when there is room, above it when there is not, and docked to the bottom of
the diff area otherwise — inset one column on each side in every case.

#### Scenario: Room below keeps the box below the line

- **WHEN** a comment is composed on a diff row with room beneath it
- **THEN** the box's top row is the row after the selected line, its left edge
  is one column inside the diff area, and its width is the diff area's width
  less two

#### Scenario: No room below flips the box above the line

- **WHEN** a comment is composed on a diff row near the bottom of the diff area
  with room above
- **THEN** the box's bottom row is the row before the selected line

#### Scenario: A selected line scrolled out of view docks the box

- **WHEN** a comment is composed while the anchored line is not among the
  rendered rows
- **THEN** the box is flush with the bottom of the diff area

#### Scenario: The renderer holds no placement arithmetic

- **WHEN** the review renderer is read
- **THEN** the compose box's position comes from the overlay layer, and the
  prefer-below/flip-above/pin-to-bottom decision appears nowhere in it
