# plugin-host/panes Specification

## Purpose
Defines how a pane a plugin declared becomes a pane a user sees — where it
sits, when it is drawn, how its content is refreshed without plugin code ever
running during a frame, and what it shows when the plugin behind it is slow or
broken.
## Requirements
### Requirement: A declared pane becomes a rendered pane

A pane declared by a running plugin's manifest SHALL be available as a pane in
the UI, titled from the manifest and placed in the slot it declared. A pane
declared by a plugin that is not running MUST NOT be shown. A pane whose
visibility is off MUST NOT be shown either, even though its plugin is running.

#### Scenario: A running plugin declares a pane

- **WHEN** a plugin with a declared, visible pane reaches `running`
- **THEN** its pane is available and titled from the manifest

#### Scenario: A failed plugin declares a pane

- **WHEN** a plugin that declared a pane fails to start
- **THEN** no pane is shown for it

#### Scenario: A running plugin's pane is hidden

- **WHEN** a running plugin's pane visibility is off
- **THEN** no pane is shown for it

### Requirement: The kernel never calls a plugin during a frame

Painting a frame SHALL NOT invoke plugin code. The kernel MUST render from a
cached view tree, and a plugin's `render` MUST run off the thread that draws.

#### Scenario: A plugin's render is slow

- **WHEN** a plugin's `render` takes far longer than a frame
- **THEN** frames continue to be painted at the normal rate from the cached
  tree

#### Scenario: A plugin's render hangs

- **WHEN** a plugin's `render` never returns
- **THEN** the UI remains responsive and the pane keeps showing its last tree

### Requirement: A pane shows its last good content while refreshing

When a re-render is requested, the pane SHALL keep displaying the previous tree
until a new one arrives. A pane that has never rendered successfully MUST show
a loading state rather than being blank or absent.

#### Scenario: A refresh is in flight

- **WHEN** a re-render has been requested and has not completed
- **THEN** the pane still shows the tree from the previous successful render

#### Scenario: A pane has never rendered

- **WHEN** a pane's plugin has not yet returned a tree
- **THEN** the pane shows a loading state

### Requirement: A render failure is contained to its pane

A `render` that errors, exceeds its execution bounds, or returns an invalid
tree SHALL leave the previous tree on screen with an error indicator, and MUST
NOT blank the frame, panic, or affect another pane.

#### Scenario: Render raises an error

- **WHEN** a plugin's `render` raises
- **THEN** its pane shows the previous tree plus an error indicator
- **AND** every other pane renders normally

#### Scenario: Render returns an invalid tree

- **WHEN** a plugin's `render` returns a value that is not a valid view tree
- **THEN** the pane shows an error indicator and the frame is unaffected

#### Scenario: The first render fails

- **WHEN** a plugin's very first `render` fails
- **THEN** the pane shows an error state rather than a loading state forever

### Requirement: A plugin pane does not defeat the demand-driven loop

A plugin pane SHALL mark the UI dirty only when its content actually changes. A
pane whose plugin returns an unchanged tree, or returns nothing new, MUST NOT
cause a repaint, so the idle paint rate is unchanged from a build with no
plugin panes.

#### Scenario: An unchanged tree

- **WHEN** a plugin re-renders and returns a tree equal to the current one
- **THEN** no repaint is triggered

#### Scenario: A changed tree

- **WHEN** a plugin returns a tree different from the current one
- **THEN** the UI is marked dirty and the pane repaints

### Requirement: Plugin panes obey the existing layout rules

A plugin pane SHALL be subject to the same width thresholds and column sharing
as the native side panels, and MUST NOT displace or overlap them.

#### Scenario: The terminal is too narrow

- **WHEN** the terminal is narrower than the threshold for side panels
- **THEN** the plugin pane is not shown, exactly as a native side panel is not

#### Scenario: A native panel is open

- **WHEN** a plugin pane and a native side panel are both visible
- **THEN** neither overlaps the other

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

