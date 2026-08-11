# layout/slots Specification

## ADDED Requirements

### Requirement: A seat's occupant is not fixed

Each seat the layout places SHALL be occupiable by the kernel's own pane **or** by
a plugin-contributed pane, and the layout MUST place the seat when either wants
it. The layout MUST NOT need to know which of the two will draw it: the flag that
carves a seat is "something occupies this", not "the kernel's pane is on".

#### Scenario: Only the kernel's pane wants the seat

- **WHEN** the kernel's own pane for a seat is on and no plugin pane claims it
- **THEN** the seat is placed, with the geometry it has always had

#### Scenario: Only a plugin pane wants the seat

- **WHEN** the kernel's own pane for a seat is off and a plugin pane claims it
- **THEN** the seat is placed, with the same geometry

#### Scenario: Nothing wants the seat

- **WHEN** neither the kernel's pane nor any plugin pane wants a seat
- **THEN** the seat is not placed and its space goes where it always went

### Requirement: The lower-left band's height is a row count, not an occupant

The band beneath the left column SHALL be sized from a **count of content rows**
supplied by whichever pane occupies it, through the existing clamp. The band MUST
remain inside the left column: hiding the left column hides the band, whoever
occupies it.

#### Scenario: The band is sized from its count

- **WHEN** the band is shown with a given content-row count
- **THEN** its height is the kernel's existing function of that count, between the
  same minimum and maximum

#### Scenario: The left column is hidden

- **WHEN** the left column is not shown
- **THEN** the band is not placed either
