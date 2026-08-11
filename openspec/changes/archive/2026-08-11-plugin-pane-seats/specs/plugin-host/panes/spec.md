# plugin-host/panes Specification

## ADDED Requirements

### Requirement: A visible plugin pane occupies its seat, and the kernel's own pane for it is not drawn

A visible pane whose slot names a single seat SHALL be drawn into that seat's
region, and the kernel's own pane for that seat MUST NOT be drawn in the same
frame. Hiding the plugin pane MUST restore the kernel's pane, and the kernel MUST
NOT lose the visibility state of its own pane while a plugin pane holds the seat.

When more than one visible pane declares the same seat, the first in publication
order SHALL take it and the others MUST NOT be drawn — a second claimant is not
placed elsewhere and does not overdraw the first.

#### Scenario: A plugin pane takes a native pane's seat

- **WHEN** a visible plugin pane declares the seat a kernel pane occupies
- **THEN** the plugin pane is drawn in that seat's rect
- **AND** the kernel's own pane for that seat is not drawn

#### Scenario: The plugin pane is hidden again

- **WHEN** a plugin pane holding a seat is hidden
- **THEN** the kernel's own pane for that seat is drawn again, in the state it was
  in

#### Scenario: Two panes claim one seat

- **WHEN** two visible plugin panes declare the same seat
- **THEN** the first in publication order is drawn there and the second is not
  drawn at all

### Requirement: A claimed seat is carved even when the kernel's pane is hidden

A seat SHALL be placed in the layout when a visible plugin pane claims it, whether
or not the kernel's own pane for that seat is toggled on. A pane whose seat the
kernel would not have carved MUST still be reachable, rather than silently never
appearing.

The seat's geometry MUST be exactly the geometry the kernel's own pane has: the
same share, the same width thresholds, the same position. With no claim, the layout
MUST be identical to one computed before seats existed.

#### Scenario: A pane claims a seat the user has toggled off

- **WHEN** a visible plugin pane claims the seat of a kernel pane that is toggled
  off
- **THEN** the seat is carved and the plugin pane is drawn in it

#### Scenario: A claimed seat keeps the native geometry

- **WHEN** a plugin pane and the kernel's own pane each occupy the same seat in
  turn
- **THEN** both are drawn into the same rect

#### Scenario: No claim changes no geometry

- **WHEN** no plugin pane claims a seat
- **THEN** every region's rect is what it was before seats existed

### Requirement: The kernel sizes a content-derived seat from the pane's own rows

Where a seat's height is a function of its content, the kernel SHALL keep that
policy for a plugin pane and derive the row count from the pane's view tree — the
number of rows its outermost stacking container holds. A plugin MUST NOT be asked
for a height, and MUST NOT be told the size it was given.

#### Scenario: A plugin pane sits in the content-sized band

- **WHEN** a visible plugin pane occupies the band whose height grows with its
  content
- **THEN** the band is sized by the kernel's existing policy applied to the number
  of rows the pane's tree stacks

#### Scenario: The pane's rows change

- **WHEN** that pane's tree stacks more rows than before
- **THEN** the band grows by the kernel's policy, up to the cap the policy already
  enforces

### Requirement: The central seat carries no kernel chrome

A plugin pane occupying the central seat SHALL be drawn with the pane frame every
plugin pane gets. The kernel's central chrome — the tab strip selecting the
kernel's own central views, and the pane-collapse affordance on its border — MUST
NOT be drawn over it, because those select surfaces that are not on screen.

#### Scenario: A plugin pane owns the centre

- **WHEN** a visible plugin pane occupies the central seat
- **THEN** it is drawn with its own titled frame
- **AND** the kernel's central tab strip is not drawn

#### Scenario: The centre is handed back

- **WHEN** that pane is hidden
- **THEN** the kernel's central view and its tab strip are drawn again
