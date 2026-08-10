# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: The open file tree is a published snapshot section

The published snapshot SHALL carry the rows of the file tree thurbox's file viewer
currently has open, as one section read through its own capability-gated reader.
The section MUST always be present — "there are no directories" is knowledge the
kernel has — so a pane iterates it without a nil check.

Each row MUST carry only what a pane draws: the node's **basename**, its depth in
the tree, whether it is a directory, whether it is expanded, and whether a running
search matched it. The section MUST also carry which row the cursor is on, and
whether nerd-font glyphs are enabled.

The kernel MUST resolve exactly what a plugin cannot observe for itself — the
user's expansion state, the cursor's row, and the search's verdict on each row —
and MUST NOT resolve what the pane owns: the marker glyphs, the indentation, and
the colour role of each row are the pane's decisions, because that mapping has one
consumer and publishing it would hand a pane the presentation it exists to own.

A row's `matched` MUST be true when no search is running, so an unsearched tree
draws in its ordinary colours without the pane having to know whether a search
exists.

#### Scenario: A tree crosses as rows of view facts

- **WHEN** a plugin holding the capability reads the section while the file viewer
  has a tree open
- **THEN** it receives one row per visible node, in the order the pane lists them,
  each carrying its basename, depth, directory flag, expansion flag and match
  verdict — and no glyph, colour or indentation

#### Scenario: The cursor's row is named once, not per row

- **WHEN** the section is read
- **THEN** the row the cursor is on is identified by its index into the rows, in
  the form a list node's selected row takes

#### Scenario: An unsearched tree reports every row as matched

- **WHEN** no search is running in the file viewer
- **THEN** every row reports itself as matched, and a pane draws them all in its
  ordinary colours

#### Scenario: A search's verdict crosses but its text does not

- **WHEN** a search is running
- **THEN** each row reports whether it matched, and the query text is not part of
  the section

### Requirement: The published file section grants no filesystem access

The file section SHALL NOT be a filesystem capability. Reading it MUST NOT let a
plugin list a directory, read a file, stat a path, or cause any I/O whatsoever:
the section is built from a tree the kernel already holds, whose shape is a record
of what the user expanded.

It MUST NOT carry a path — neither a root's nor a node's, neither absolute nor
relative. A row carries a basename; the tree's shape follows from depth, which is
inherent to drawing a tree, and reveals nothing about where on disk the tree is.

It MUST NOT contain a node the user has not expanded, a hidden file, or anything
outside the active session's own directories.

#### Scenario: A plugin holding the capability cannot read the filesystem

- **WHEN** a plugin declares the file capability and nothing else
- **THEN** its module table contains the file reader and no binding that lists a
  directory or reads a file

#### Scenario: A row carries no path

- **WHEN** a plugin reads the section
- **THEN** no row carries a path, and no field reveals the location of the tree on
  disk

#### Scenario: Unexpanded directories are absent

- **WHEN** a directory in the tree has not been expanded
- **THEN** its children are not in the section, and reading the section does not
  cause them to be read from disk

### Requirement: The published file section is bounded and respects its feature

The number of rows published SHALL be bounded, so that a tree with a large
directory expanded in it cannot produce a view tree beyond the node budget — which
would make every render of a file pane *fail* rather than merely scroll.

When more rows exist than the bound allows, the section MUST publish the first
rows up to the bound and MUST NOT publish a cursor index that falls outside them:
an index into rows that were not published would make the kernel's own windowing
meaningless.

The section MUST be empty when the file-viewer feature is disabled, mirroring how
the task and automation sections respect theirs — thurbox draws no file viewer in
that configuration, so a pane advertising one would surface a disabled feature.

#### Scenario: A very large tree is truncated rather than failing a render

- **WHEN** the open tree has more visible rows than the bound
- **THEN** the section carries the bound's worth of rows and a pane built from it
  renders

#### Scenario: A cursor beyond the bound is not published

- **WHEN** the cursor is on a row past the bound
- **THEN** the section publishes no cursor index

#### Scenario: The feature is off

- **WHEN** the file-viewer feature is disabled
- **THEN** the section is empty
