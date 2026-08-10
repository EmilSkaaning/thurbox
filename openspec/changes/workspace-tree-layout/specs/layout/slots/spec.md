## MODIFIED Requirements

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
