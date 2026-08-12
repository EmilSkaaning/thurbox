# plugin-host/mouse delta

## MODIFIED Requirements

### Requirement: A click on a plugin pane resolves to one of its rows

The host SHALL make each row of a plugin pane's **outermost list** clickable, and
a click inside a row's rect MUST resolve to that row's index within the list,
counted from one — the same numbering a list uses to declare which row its cursor
is on.

The outermost list, not every list: the pane's rows are what a user points at, and
a nested list's rows would give one click two answers. A pane whose tree contains
no list has no clickable rows, and a click on it does nothing beyond focusing the
pane.

A row's rect MUST span every line that row renders as. A list's child is one row
however many lines it stacks, so a click anywhere in a multi-line child — on the
heading above a record as much as on the record — MUST report that one child's
index, and MUST NOT be split into an index per line or fall through to the child
below.

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

#### Scenario: A click on any line of a multi-line row

- **WHEN** a list's children each stack a heading line above a content line, and a
  click lands on the heading of the second child
- **THEN** row two is reported, the same index a click on that child's content
  line reports
