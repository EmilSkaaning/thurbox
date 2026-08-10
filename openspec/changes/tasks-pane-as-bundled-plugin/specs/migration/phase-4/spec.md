# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A second native pane is reproduced by a bundled plugin

A second of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, and leaving the native
pane as the one the interface draws.

The chosen pane is the **tasks pane**, because it is the first *list* pane — a
selectable list with search emphasis, which is the shape every remaining Phase 4
pane has — so what it needs is what those ports will need.

#### Scenario: The second pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the tasks pane's plugin is discoverable, its manifest satisfies the
  same validation a user's plugin does, and its pane is off screen until asked
  for

#### Scenario: Both bundled panes coexist

- **WHEN** both bundled panes are put on screen
- **THEN** each renders its own pane, and neither native renderer is replaced

### Requirement: A list pane's row styling is expressible without naming a colour

A pane reproduced as a plugin SHALL be able to draw a selectable list row in
every appearance the native pane gives it — selected, filtered out by a running
search, and with matched characters emphasised — using only the declared style
vocabulary. If it cannot, the vocabulary MUST be widened in the same change
rather than the pane approximating one appearance with another.

#### Scenario: Three row appearances are reproduced exactly

- **WHEN** the native pane and the plugin are given a list containing a selected
  row, a row filtered out by a search, and a row with matched characters
- **THEN** the two view trees are equal, so each row is drawn identically

#### Scenario: The pane still names no colour

- **WHEN** the plugin's rows are inspected
- **THEN** every one is styled by token and emphasis, and none names a colour

### Requirement: A pane whose rows depend on geometry keeps that geometry in the kernel

When a native pane's rows depend on its resolved size — fitting a label to the
column, reserving room for a trailing marker, scrolling a window to keep the
selection visible — the port SHALL leave that resolution in the kernel rather
than reporting a rect into a plugin. The plugin's copy of the pane MUST therefore
be allowed to differ in exactly those respects, and each difference MUST be
pinned by its own test naming what the plugin does instead and what would close
it.

A port MUST NOT hide such a difference by publishing rows already fitted to
another pane's size: the plugin's pane is a different rect, so rows fitted
elsewhere would be wrong at its own size.

#### Scenario: Rows fit in the column

- **WHEN** every row fits the width and the list fits the height
- **THEN** the native pane's tree and the plugin's are equal

#### Scenario: A row is wider than the column

- **WHEN** a row's label exceeds the native pane's width
- **THEN** the native pane fits it and the plugin's copy does not, and a test
  asserts that difference and names the node that would remove it

#### Scenario: The list is longer than the pane

- **WHEN** there are more rows than the pane has lines
- **THEN** the native pane windows them around the selection while the plugin's
  copy draws from the first row, and a test asserts that difference
