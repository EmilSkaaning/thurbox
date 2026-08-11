# kernel-state (delta)

## MODIFIED Requirements

### Requirement: The task list is a published snapshot section

The published kernel-state snapshot SHALL carry the task list as its own
section, read by its own capability-gated binding. Each published row MUST carry
the task's title, its status as a stable wire name, and the view facts the kernel
owns because it owns the keyboard and the search: whether the row is the selected
one, whether a running search filtered it out, which characters that search
matched, and whether the task has an open related session.

The section MUST also carry the index of the row the pane's **cursor** is on,
separately from the per-row selected flag, because the two answer different
questions:

- the cursor index is a **scroll anchor**: it says which row a pane hands to its
  list so the kernel can keep that row in view, and it MUST be published
  regardless of which pane holds focus — a list has to scroll to its cursor
  whether or not the cursor is currently being drawn;
- the per-row selected flag is an **appearance**: it says this row is drawn as
  the cursor's, and it stays gated on the cursor being visible (the task pane
  holds focus, or a search preview is moving it).

Publishing one and deriving the other MUST NOT be attempted in either direction.
A pane deriving the anchor from the appearance would stop scrolling the moment
the native pane lost focus, and a pane deriving the appearance from the anchor
would draw a cursor thurbox is not drawing.

The index MUST be absent rather than zero when there is no cursor to name, which
is the same "absent means absent" rule the rest of the boundary uses.

The section MUST NOT carry the glyph or the colour a status is drawn with.
Publishing them would hand a pane the presentation it exists to own — unlike a
*session* status, whose glyph and style token the kernel resolves because that
mapping is shared by more than one native pane and a second copy of it could
disagree.

The section MUST be present whenever the snapshot is, so a plugin needs no
absence check to iterate it: "there are no tasks" is knowledge the kernel has.

#### Scenario: A plugin reads the task rows

- **WHEN** a snapshot carrying task rows is published and a plugin holding the
  task capability calls its reader
- **THEN** it receives one entry per row, in the kernel's order, each carrying
  the title, the status wire name, and the selection, search and linkage facts

#### Scenario: A plugin scrolls its copy to the cursor

- **WHEN** the published list is longer than the plugin's pane has lines and the
  cursor is below the fold
- **THEN** the section's cursor index names that row, the plugin hands it to its
  list, and the kernel draws the slice containing it

#### Scenario: The cursor is published while another pane holds focus

- **WHEN** the task pane does not hold focus
- **THEN** the section still carries the cursor index, while no row is marked as
  the cursor's

#### Scenario: There is no cursor to name

- **WHEN** the task list is empty
- **THEN** the section carries no cursor index

#### Scenario: There are no tasks

- **WHEN** the task list is empty
- **THEN** the reader returns an empty collection rather than nothing, so a
  plugin iterates it without a nil check

#### Scenario: A status crosses as a name, not as a rendering

- **WHEN** a plugin reads a task row
- **THEN** the row names its status and carries neither a glyph nor a style
  token, so the pane chooses both

#### Scenario: A search's verdict crosses as offsets

- **WHEN** a search is running and a row matched it
- **THEN** the row carries the matched character offsets, and the pane decides
  how a matched run is emphasised

### Requirement: The published task section is bounded and respects its feature

The publisher SHALL publish no more task rows than a pane can render, so that a
list larger than the view tree's node budget cannot make every render fail. The
bound MUST be a property of the published section rather than of any one
consumer.

When more rows exist than the bound allows, the section MUST NOT publish a
cursor index that falls outside the rows it published: an index into rows a pane
never received would make the kernel's own windowing meaningless. This is the
rule the file section already states, and it holds for the same reason.

When the task feature is disabled the section MUST be empty, because thurbox
shows no task list at all in that configuration and a pane advertising one would
surface a disabled feature.

#### Scenario: More tasks than a pane can draw

- **WHEN** the task list is longer than the published bound
- **THEN** the snapshot carries at most that many rows and the reader stays
  usable

#### Scenario: A cursor beyond the bound is not published

- **WHEN** the cursor is on a row past the published bound
- **THEN** the section publishes no cursor index

#### Scenario: The task feature is off

- **WHEN** tasks are disabled in settings
- **THEN** the published section is empty
