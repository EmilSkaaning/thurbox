# layout/slots Specification

## Purpose
Describes how the TUI decides which panels are visible and where they sit, as a
slot model rather than a fixed field per panel — so that adding a pane is a
value, not a change to the layout function's signature.
## Requirements
### Requirement: Layout inputs are a named structure

The layout SHALL take its inputs as a named structure rather than a positional
argument list. Adding a new panel MUST NOT change the arity of the layout
entry point or require editing unrelated call sites.

#### Scenario: A new panel is introduced

- **WHEN** a new panel type is added to the layout
- **THEN** existing call sites that do not use it compile unchanged

#### Scenario: Defaults are explicit

- **WHEN** a caller supplies no preference for a panel
- **THEN** that panel is absent, and the layout is identical to one where it
  was explicitly disabled

### Requirement: A column holds an ordered list of occupants

A layout column SHALL be a **branch of the workspace tree** whose children are
its occupants in draw order, rather than a fixed set of named rects. Occupants
MUST be laid out in that order, an absent occupant MUST NOT leave a gap, and the
list MUST accept **any number** of plugin-contributed occupants rather than a
single one.

Occupant order is decided by the host — the tasks panel, then the file viewer,
then the plugin panes in publication order — so two panes can never disagree
about which comes first.

#### Scenario: Two occupants share a column

- **WHEN** two panels occupy the same column
- **THEN** they appear in list order and do not overlap

#### Scenario: One occupant is hidden

- **WHEN** one of several occupants is not shown
- **THEN** the remaining occupants fill the column with no gap where it was

#### Scenario: Several plugin panes occupy the column

- **WHEN** more than one plugin pane is visible
- **THEN** each gets its own occupant region in the column, in publication
  order, after the native occupants

### Requirement: Existing geometry is preserved exactly

Converting the layout to a slot model SHALL NOT change any panel's position or
size at any terminal dimension. The rendered frame MUST be identical to the
previous implementation for every combination of visible panels.

#### Scenario: Pinned screens are unchanged

- **WHEN** the acceptance snapshots are rendered after the conversion
- **THEN** every snapshot matches its previous content exactly

#### Scenario: Width thresholds are unchanged

- **WHEN** the terminal is at each existing panel-visibility threshold
- **THEN** the same panels are shown as before the conversion

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

