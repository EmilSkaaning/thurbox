## Purpose

Describes how the TUI decides which panels are visible and where they sit, as a
slot model rather than a fixed field per panel — so that adding a pane is a
value, not a change to the layout function's signature.

## ADDED Requirements

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

A layout column SHALL be described as an ordered list of occupants rather than
a fixed set of named rects. Occupants MUST be laid out in list order, and an
absent occupant MUST NOT leave a gap.

#### Scenario: Two occupants share a column

- **WHEN** two panels occupy the same column
- **THEN** they appear in list order and do not overlap

#### Scenario: One occupant is hidden

- **WHEN** one of several occupants is not shown
- **THEN** the remaining occupants fill the column with no gap where it was

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
