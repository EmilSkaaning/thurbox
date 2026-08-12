# plugin-host/panes delta

## ADDED Requirements

### Requirement: A pane's frame reports the rows the host clipped

Where the host windows a pane's list to fit the seat, it SHALL report on that pane's
frame how many items it clipped above and below — the counts painted on the top and
bottom border rows, right-aligned, in the same form thurbox's own panes have always
used.

The counts MUST be read off the paint that produced them, not from a declaration and not
from a second traversal: the window is the host's rule and the frame is the host's, so a
plugin can neither ask for the indicator nor disagree with it. A plugin MUST NOT be told
either count, because it is told no geometry at all.

The counts SHALL be **items**, so a list child that stacks several lines counts once —
which is what a reader counting rows they cannot see means.

This applies to every pane the host paints, seated or not: hiding rows silently is a
property of the host's window, so a pane whose renderer is a plugin MUST NOT be worse
off than one of thurbox's own.

#### Scenario: A list that overflows its pane

- **WHEN** a plugin pane's list has more items than the seat has rows, and the window
  starts past the first item
- **THEN** the number of items above the window is drawn on the pane's top border and the
  number below it on the bottom border

#### Scenario: A list that fits

- **WHEN** every item of a plugin pane's list is on screen
- **THEN** no indicator is drawn on either border

#### Scenario: The indicator costs no content row

- **WHEN** the same tree is painted into the same seat with and without items clipped
- **THEN** the content area and the reported row hitboxes are identical in both

#### Scenario: A two-line item counts once

- **WHEN** a clipped item stacks a header above its row
- **THEN** it contributes one to the count, not two
