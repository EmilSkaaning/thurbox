# plugin-host/panes Specification

## ADDED Requirements

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
