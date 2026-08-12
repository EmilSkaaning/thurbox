# plugin-host/panes Specification

## ADDED Requirements

### Requirement: A kernel surface that owns a seat preempts the pane holding it

Where a seat has a second occupant that is one of thurbox's own transient surfaces, that
surface SHALL take the seat for as long as it is present, and the plugin pane holding the
seat MUST NOT be painted for that time. This reverses, for such a seat only, the rule that
a visible plugin pane occupies its seat.

The preemption MUST be the kernel's decision, resolved from the kernel's own state. A
manifest MUST NOT be able to declare that a pane preempts another, or that it yields to
one: a plugin cannot see thurbox's surfaces, and a declared precedence would let one
independently-written manifest outrank another with nothing able to arbitrate.

The preempted pane MUST be told nothing. It keeps rendering on its own schedule, it is
never informed that it was not painted, and — critically — its **stored visibility MUST
be unchanged**, so that when the transient surface goes the pane returns to the screen
with no user action.

While preempted, the seat's focus, keys and clicks MUST belong to the surface that is
drawn there, not to the pane that is not.

#### Scenario: The transient surface appears

- **WHEN** a kernel surface that preempts a seat becomes present while a plugin pane holds
  that seat
- **THEN** the surface is drawn in the seat and the pane is not painted

#### Scenario: The transient surface goes

- **WHEN** that surface is no longer present
- **THEN** the plugin pane is painted in the seat again, with the visibility it had, and
  with no user action

#### Scenario: A preempted pane's keys go to the surface that is drawn

- **WHEN** the seat is preempted and the user focuses that column
- **THEN** focus, keys and clicks are the drawn surface's, and none reaches the pane

#### Scenario: A manifest tries to declare precedence

- **WHEN** a manifest attempts to declare that its pane preempts or yields to another
- **THEN** there is no such field to declare, and precedence remains resolved by the
  kernel

## MODIFIED Requirements

### Requirement: A pane's content area is the seat minus the kernel's chrome

A pane's tree SHALL be laid out in the seat's area minus any chrome the kernel draws
there. The plugin MUST NOT be told either area — it is told no geometry at all — so
reserving space for chrome MUST NOT change what the plugin returns, only where the
kernel paints it.

Chrome MAY be a single row **inside** the pane's frame or a bordered band of several rows
**outside** it, whichever the pane it replaces drew. Where the chrome is a band outside
the frame, the band MUST be subtracted from the seat **before** the frame is drawn, so
that the pane's frame, its content area and its row hitboxes are the ones the native
pane's content had.

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

#### Scenario: The chrome is a bordered band below the frame

- **WHEN** a seat's chrome is a multi-row bordered band that the native pane drew below
  its frame
- **THEN** the band is drawn in that position and the pane's frame occupies the rest of
  the seat, so the pane's box is the size it was before the handover
