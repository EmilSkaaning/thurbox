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

### Requirement: A pane is rendered when a source it reads moves

A pane's re-render SHALL be triggered by a change in something the pane can read,
not by the passage of time. The host MUST know, per pane, which sources its
plugin's granted capabilities reach, and MUST render only the panes whose sources
moved.

A source is one of the published snapshot's sections — the sessions, the host
metrics, the automations, the tasks, the open file tree, the open review — or the
plugin's own durable state. Each state-reading capability SHALL name exactly one
source, and the mapping MUST be exhaustive over the capability vocabulary, so a
capability added later cannot reach a pane without declaring what it reads.

A pane MUST also be rendered when its plugin was offered input (its own state may
have moved in answering), when it has just become visible after being skipped, and
when its plugin has been reloaded.

A pane that the kernel publishes as hidden MUST NOT be rendered whatever moved, and
a change in a source no visible pane reads MUST cost no render at all.

#### Scenario: A source a pane reads moves

- **WHEN** the state behind a source changes and a visible pane's plugin holds the
  capability that reads it
- **THEN** that pane is re-rendered

#### Scenario: A source no visible pane reads moves

- **WHEN** the state behind a source changes and no visible pane's plugin holds the
  capability that reads it
- **THEN** no pane is rendered and no plugin VM is entered

#### Scenario: Nothing moves

- **WHEN** no source a visible pane reads changes
- **THEN** no pane is rendered, however long the worker runs

#### Scenario: A pane becomes visible

- **WHEN** a pane the worker was skipping is shown
- **THEN** it is rendered rather than waiting for a source to move

#### Scenario: A plugin is offered input

- **WHEN** a key or a click is offered to a pane's plugin
- **THEN** that pane is re-rendered, because answering may have changed the
  plugin's own state

#### Scenario: A capability is added to the vocabulary

- **WHEN** a new capability is added
- **THEN** the source mapping fails to compile until it names the source that
  capability reads, or records that it reads none

### Requirement: A pane's render rate is bounded, and the bound is coalescing only

The host SHALL bound how often a pane is rendered, at no more than ten render
passes per second, and MUST reach that bound by **coalescing** rather than by
delaying: a change that arrives after the bound's interval has elapsed MUST be
rendered immediately, and changes arriving inside it MUST be merged into one pass
at its end.

A bound is required because a source can move far faster than a user can perceive —
agent activity text can change on consecutive ticks — and an unbounded trigger would
put a plugin VM call on every one of them. It MUST NOT be reached by rendering on a
timer, because that is the fixed cadence this requirement replaces.

The bound MUST be no looser than the kernel's own forced-redraw floor, so a plugin
pane cannot be more than one forced frame behind the interface around it.

The residual latency the bound introduces SHALL be recorded rather than described as
zero.

#### Scenario: A change after a quiet period

- **WHEN** a source moves and no pane has been rendered within the bound's interval
- **THEN** the render happens immediately, with no wait

#### Scenario: A source moving faster than the bound

- **WHEN** a source changes on many consecutive ticks
- **THEN** the changes coalesce and the pane is rendered at most ten times a second

#### Scenario: A change inside the interval

- **WHEN** a source moves shortly after a render pass
- **THEN** the pane is rendered once the interval has elapsed, and the delay is
  recorded as the trigger's worst case

### Requirement: A pane whose source the kernel cannot observe is the only pane on a timer

A pane that may read its plugin's **own durable state** SHALL keep a periodic
re-render, because that state has no observable change event: a plugin's headless
half can write it with nothing on the UI thread knowing. Every other pane MUST have
no timer at all.

The periodic re-render MAY share the cadence of the source-file poll the host
already runs for reload, since both exist for the same reason — a change the kernel
cannot be told about. It MUST NOT be raised when no running plugin declares the
capability that reads that state, so a set of plugins that read only published state
costs zero idle renders.

#### Scenario: A pane reads its plugin's own state

- **WHEN** a visible pane's plugin holds the capability that reads its own durable
  state
- **THEN** that pane is re-rendered periodically, whether or not any published
  source moved

#### Scenario: No plugin reads its own state

- **WHEN** no running plugin holds that capability
- **THEN** no periodic re-render is raised and an idle host enters no plugin VM

### Requirement: A pane that holds focus is drawn as focused

A plugin pane's frame SHALL show whether the pane holds focus, using the same
appearance thurbox's own panes use for it. A focusable pane drawn identically
whether or not it is focused would leave a user unable to see where their keys are
going.

The appearance SHALL be resolved by the kernel from the focus it owns, not published
to the plugin and not declared in the tree: a plugin is told nothing about its own
focus, and a frame is the host's.

For a pane that declared one of the kernel's pane keyboards, the level SHALL be the
level the kernel's own pane for that keyboard would have been drawn with, resolved
by one shared rule — including any intermediate level such a pane has while a
surface it opened holds focus. Two rules for one appearance is how a handed-over
pane comes to look subtly unlike the pane it replaced.

#### Scenario: A focusable pane holds focus

- **WHEN** a plugin pane that can receive keys holds focus
- **THEN** its frame is drawn as focused

#### Scenario: A focusable pane does not hold focus

- **WHEN** a plugin pane that can receive keys does not hold focus
- **THEN** its frame is drawn as unfocused

#### Scenario: A pane that cannot receive keys

- **WHEN** a plugin pane that can never receive keys is drawn
- **THEN** its frame is drawn as unfocused, exactly as before

### Requirement: A pane's content area is the seat minus the kernel's chrome

A pane's tree SHALL be laid out in the seat's area minus any chrome the kernel draws
there. The plugin MUST NOT be told either area — it is told no geometry at all — so
reserving a row for chrome MUST NOT change what the plugin returns, only where the
kernel paints it.

A pane's clickable rows MUST be reported against the area the tree was actually
painted into, so that a click on the pane's *n*th visible row selects its *n*th row
whether or not chrome is present.

#### Scenario: Chrome is present

- **WHEN** the kernel draws a chrome row in a pane's seat
- **THEN** the pane's tree is painted into the rest of the seat and its row hitboxes
  are inside that area

#### Scenario: Chrome is absent

- **WHEN** no chrome is drawn for that seat
- **THEN** the pane's tree is painted into the whole seat

