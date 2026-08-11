# layout/slots Specification

## MODIFIED Requirements

### Requirement: A seat's occupant is not fixed

Each seat the layout places SHALL be occupiable by the kernel's own pane **or** by
a plugin-contributed pane, and the layout MUST place the seat when either wants
it. The layout MUST NOT need to know which of the two will draw it: the flag that
carves a seat is "something occupies this", not "the kernel's pane is on".

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
