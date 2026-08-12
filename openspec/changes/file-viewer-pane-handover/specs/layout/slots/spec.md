# layout/slots Specification

## MODIFIED Requirements

### Requirement: A seat's occupant is not fixed

Each seat the layout places SHALL be occupiable by the kernel's own pane **or** by
a plugin-contributed pane, and the layout MUST place the seat when either wants
it. The layout MUST NOT need to know which of the two will draw it: the flag that
carves a seat is "something occupies this", not "the kernel's pane is on".

Every seat a pane of thurbox's own occupies SHALL be nameable by a manifest — including
the seats **inside a column**. A column already seats a plugin pane, but a position
in it is part of the pane: a column's occupants are drawn in a fixed order, so a pane
seated as "another occupant of the column" lands beside the position the pane it
replaces had rather than in it. A handover that moved a pane one column over would be
a change a user notices, so the seat is named rather than approximated.

A seat MAY have **exactly one** possible occupant. When a pane is handed over, its
seat's kernel occupant is deleted and the seat's flag becomes the plugin claim
alone. The layout MUST need no change for this — that the flag means "something
occupies this" is what makes a seat with one occupant a value rather than a branch —
and the seat's geometry MUST be unchanged, since the seat is the same seat with a
different painter.

A seat with no possible occupant MUST NOT be carved. This is the case a retained
kernel flag would break: a `bool` kept after its renderer was deleted can still say
"something occupies this" when nothing does, and the layout would place a column no
one paints.

A seat MAY have a **second kernel occupant** — a transient kernel surface that takes the
seat for as long as it is present, such as a review's changed-files list occupying the
file viewer's column for as long as a review is open. Where one exists, the seat's carve
condition SHALL be the disjunction "a pane claims it **or** the transient surface is
present", so the column is placed for either occupant with the geometry it has always
had. The transient surface's presence MUST NOT change the seat's size or position: it is
the same seat with a different painter, exactly as a handover is.

#### Scenario: Only the kernel's pane wants the seat

- **WHEN** the kernel's own pane for a seat is on and no plugin pane claims it
- **THEN** the seat is placed, with the geometry it has always had

#### Scenario: Only a plugin pane wants the seat

- **WHEN** the kernel's own pane for a seat is off and a plugin pane claims it
- **THEN** the seat is placed, with the same geometry

#### Scenario: Nothing wants the seat

- **WHEN** neither the kernel's pane nor any plugin pane wants a seat
- **THEN** the seat is not placed and its space goes where it always went

#### Scenario: A seat whose only occupant is a plugin pane

- **WHEN** a seat's kernel occupant has been deleted by a handover and a plugin pane
  claims the seat
- **THEN** the seat is placed with the geometry it had, and when no pane claims it
  the seat is not placed at all

#### Scenario: A seat inside a column

- **WHEN** a plugin pane claims the seat of a pane that sits inside a column
- **THEN** it is drawn in that pane's position in the column, not appended after the
  column's other occupants

#### Scenario: A transient kernel surface wants a seat no pane claims

- **WHEN** a seat's transient kernel occupant is present and no plugin pane claims the
  seat
- **THEN** the seat is placed with its usual geometry, so the transient surface is drawn
  where that column has always been

#### Scenario: A transient kernel surface and a claim want the seat at once

- **WHEN** a seat's transient kernel occupant is present and a plugin pane also claims
  the seat
- **THEN** the seat is placed exactly once, with the same geometry, and which of the two
  is painted is decided by the precedence rule rather than by the layout
