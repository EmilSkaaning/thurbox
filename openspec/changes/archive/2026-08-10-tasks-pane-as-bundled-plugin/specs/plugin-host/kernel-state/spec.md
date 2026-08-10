# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: The task list is a published snapshot section

The published kernel-state snapshot SHALL carry the task list as its own
section, read by its own capability-gated binding. Each published row MUST carry
the task's title, its status as a stable wire name, and the view facts the kernel
owns because it owns the keyboard and the search: whether the row is the selected
one, whether a running search filtered it out, which characters that search
matched, and whether the task has an open related session.

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

When the task feature is disabled the section MUST be empty, because thurbox
shows no task list at all in that configuration and a pane advertising one would
surface a disabled feature.

#### Scenario: More tasks than a pane can draw

- **WHEN** the task list is longer than the published bound
- **THEN** the snapshot carries at most that many rows and the reader stays
  usable

#### Scenario: The task feature is off

- **WHEN** tasks are disabled in settings
- **THEN** the published section is empty
