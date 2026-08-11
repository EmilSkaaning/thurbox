# plugin-host/mouse Specification

## ADDED Requirements

### Requirement: A click on a plugin pane resolves to one of its rows

The host SHALL make each row of a plugin pane's **outermost list** clickable, and
a click inside a row's rect MUST resolve to that row's index within the list,
counted from one — the same numbering a list uses to declare which row its cursor
is on.

The outermost list, not every list: the pane's rows are what a user points at, and
a nested list's rows would give one click two answers. A pane whose tree contains
no list has no clickable rows, and a click on it does nothing beyond focusing the
pane.

When the kernel has scrolled a list — which it does when the list names a selected
row and has more rows than the pane has lines — the reported index MUST be the
row's index in the **whole** list, not its position on screen. A plugin's rows and
the kernel's window are different things, and the plugin only knows its own.

#### Scenario: A click on a row

- **WHEN** a plugin pane draws a list and a click lands inside its third row
- **THEN** the plugin is told that row three of that pane was clicked

#### Scenario: A click on a row of a scrolled list

- **WHEN** a list is scrolled so its first visible row is not its first row, and
  the top visible row is clicked
- **THEN** the reported index is that row's index in the whole list

#### Scenario: A pane with no list

- **WHEN** a plugin pane's tree contains no list and it is clicked
- **THEN** no row is reported

#### Scenario: A click outside every row

- **WHEN** a click lands on a plugin pane below its last row
- **THEN** no row is reported, and the pane is still focused

### Requirement: A click carries a row and nothing about geometry

The host SHALL report the pane and the row index, and MUST NOT report a
coordinate, a rect, a width, a height, or a screen position. A plugin learns
*which row* was clicked and never *where*.

This is the same refusal the view tree makes: a plugin that knew its geometry
would render width-dependently, and a resize would have to re-enter its VM before
the frame that needs it.

#### Scenario: What a click carries

- **WHEN** a click is delivered to a plugin
- **THEN** it carries the pane's id and a row index, and no positional value

### Requirement: Only a plugin that declared input receives a click

A click SHALL be delivered only to a plugin that declared the input capability,
and only for a pane that is on screen. A pane whose plugin did not declare it MUST
NOT be focused by a click and MUST NOT be told about one — the same rule focus
navigation already applies.

A click is input; it is gated by the capability that gates input rather than by a
capability of its own.

#### Scenario: A pane without the input capability is clicked

- **WHEN** a visible plugin pane whose plugin lacks the input capability is
  clicked
- **THEN** nothing is delivered and focus does not move to it

#### Scenario: A pane with the input capability is clicked

- **WHEN** a visible plugin pane whose plugin declared input is clicked
- **THEN** the click is delivered to that plugin

### Requirement: Focus follows the click, and names the pane it landed on

Clicking a focusable plugin pane SHALL focus that pane, so the keys that follow go
to the pane the user pointed at. When several focusable plugin panes are on
screen, the focused one MUST be the one that was clicked, not the first one
declared.

#### Scenario: Clicking focuses the pane

- **WHEN** a focusable plugin pane is clicked while another pane holds focus
- **THEN** the plugin pane holds focus afterwards

#### Scenario: Clicking the second of two panes

- **WHEN** two focusable plugin panes are on screen and the second is clicked
- **THEN** a key pressed afterwards is delivered to the second pane

### Requirement: A click never blocks the frame, and an unconsumed one does nothing

Delivering a click SHALL NOT block the thread that draws frames for longer than
the bound a key already carries, and a plugin that fails or hangs while handling
one MUST cost the click rather than the interface.

A click the plugin does not consume MUST have no further effect: unlike an
unconsumed key, there is nothing for thurbox to fall through to — the pane the
user pointed at is the plugin's.

#### Scenario: A slow click handler

- **WHEN** a plugin does not answer a click within the host's bound
- **THEN** the frame loop continues and the click is treated as unconsumed

#### Scenario: No click handler

- **WHEN** a plugin declares input but defines no click handler
- **THEN** nothing fails and the click is not consumed

#### Scenario: An unconsumed click

- **WHEN** a plugin reports a click unconsumed
- **THEN** thurbox does not act on it in the plugin's place
