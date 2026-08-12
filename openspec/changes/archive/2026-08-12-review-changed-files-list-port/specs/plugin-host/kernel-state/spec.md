# plugin-host/kernel-state delta

## ADDED Requirements

### Requirement: The review's changed-files tree is a second published list

The published **review** section SHALL carry the review's **changed-files tree** —
the list the native pane draws beside the diff — as a second list, in the order the
pane lists it, alongside which of its rows holds the file the diff's cursor is in.

It MUST be published rather than left to the pane to derive from the row stream, and
for two independent reasons:

- the row stream is **bounded**, so a pane counting file headers in it would list a
  prefix of the changed files and present it as the set;
- the tree's **order is not the stream's** — files are grouped by directory and
  sorted by path segment — so a pane sorting for itself would diverge on the first
  pair of paths whose ordering is not obvious, and the two panes would disagree
  about which file a row names.

The rows are a closed pair of kinds, each carrying only what the pane cannot derive:

- a **folder** row: its indentation depth and the directory segment's own name;
- a **file** row: its indentation depth, its path within the review, its status as a
  stable wire name, its insertion and deletion counts, and whether it is marked
  reviewed.

The cursor MUST be the row of **this** list, not of the row stream, and MUST be
absent when the diff's cursor is on a row belonging to no file — where the native
pane highlights nothing and opens its window at the top.

The section MUST NOT carry the row's rendering: not the basename the row draws, not
the status glyph, not the reviewed mark, and not the indent as spaces. Each is
derived by the pane from a fact it was given, exactly as a file header's glyph is.

This list SHALL be bounded like every other, and the bound MUST be separate from the
row stream's: a changed-file row costs a fixed handful of nodes where a diff line's
body costs one per token, so the two bounds answer different questions and a shared
number would be wrong for both. Past the bound the section carries the first rows
and no cursor.

The bound MUST NOT be imposed on a **kernel** pane drawing the same list locally: it
holds the whole review, so it lists every file. The difference between the two is a
divergence the reproduction enumerates rather than a rule either side breaks.

#### Scenario: A pane reads the changed-files tree

- **WHEN** a review is open and a plugin holding the capability reads the section
- **THEN** it receives one entry per tree row, directory headers included, in the
  order the native pane lists them

#### Scenario: The tree outlives the stream's bound

- **WHEN** a review has more rows than the stream's bound and its later files never
  appear in the published stream
- **THEN** those files still appear in the changed-files tree, because the two lists
  are bounded separately

#### Scenario: The cursor is a row of this list

- **WHEN** the diff's cursor moves to a line inside the third changed file
- **THEN** the published changed-files cursor names that file's row in the tree, not
  the diff row's index

#### Scenario: The cursor belongs to no file

- **WHEN** the diff's cursor is on the review-summary section
- **THEN** the changed-files cursor is absent, and a pane draws no highlight

#### Scenario: No rendering crosses

- **WHEN** the published rows are inspected
- **THEN** a file row carries its path and status name, and carries no basename, no
  status letter, no reviewed glyph and no indentation string
