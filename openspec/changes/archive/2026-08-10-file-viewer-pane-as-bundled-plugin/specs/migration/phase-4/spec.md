# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A third native pane is reproduced by a bundled plugin

A third of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first two: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, off screen by default,
and leaving the native pane as the one the interface draws.

The chosen pane is the **file viewer's tree**, because it is the pane that cannot
accept the scrolling gap the previous port recorded: its entire interaction is
moving a cursor through a tree taller than its column, so a copy that draws from
the first row is not a reproduction of it.

#### Scenario: The third pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the file viewer's plugin is discoverable, its manifest satisfies the
  same validation a user's plugin does, and its pane is off screen until asked for

#### Scenario: The reproduction is equal to the native tree

- **WHEN** the native pane and the plugin are given the same tree
- **THEN** the two view trees are equal, across collapsed and expanded
  directories, nested depths, both marker glyph sets, a running search, a selected
  row, and an empty tree

### Requirement: A pane's scroll window is resolved by the kernel from a declared selection

When a ported pane's list is longer than the rows it has, the scroll window SHALL
be resolved by the kernel from a selection the plugin declares — not by reporting
the pane's resolved height into the plugin, and not by publishing rows already
windowed to another pane's size.

The rule that resolves it MUST be the same one thurbox's own panes use, so that a
native pane and the plugin reproducing it are not merely equal as trees but paint
the same frame when the pane scrolls.

This closes the second of the two geometry gaps the previous port recorded, for
every remaining pane rather than only this one.

#### Scenario: The plugin's pane scrolls to its cursor

- **WHEN** the tree has more rows than the pane has lines and the cursor is below
  the fold
- **THEN** the plugin's pane draws the slice containing the cursor, the same slice
  the native pane draws

#### Scenario: The plugin still learns no dimension

- **WHEN** the plugin's rendering is inspected
- **THEN** nothing in it consults a width or a height, and the rows it returns are
  the whole list

### Requirement: A pane sub-mode the host surface cannot express is declared out of scope

When part of a pane cannot be reproduced because the host surface cannot express
it, the port SHALL declare that part out of scope **in its proposal**, naming the
host features that are missing, rather than omitting it silently. The parts of the
same behaviour that *are* expressible MUST still be ported, so the record
distinguishes "cannot be drawn" from "was not attempted".

#### Scenario: The unexpressible part is named with what it needs

- **WHEN** the file viewer's search bar cannot be drawn
- **THEN** the proposal states it is out of scope and the readiness document names
  the missing host features, one per missing capability of the surface

#### Scenario: The expressible part of the same behaviour is still ported

- **WHEN** a search is running
- **THEN** the plugin's tree draws the search's effect on every row — matched and
  unmatched — identically to the native pane, even though the search bar is absent
