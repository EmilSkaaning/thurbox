# migration/phase-4 delta

## ADDED Requirements

### Requirement: The code review's changed-files list windows by the kernel's rule

The changed-files list SHALL resolve its visible rows, its click hitboxes and its
selection appearance through the kernel's shared painter and the kernel's shared
windowing rule, and SHALL NOT compute a window of its own.

The pane's folder-tree grouping — which directories head which files, at which depth, and
which file each row names — SHALL live in the layer that owns the diff's data rather than
in the pane that draws it, so a second drawer of this list resolves the same rows rather
than reimplementing the sort. The rows MUST name each file by its index into the review's
own file list, so a row survives crossing to a reader that holds no diff.

The list's selection appearance SHALL be carried by the row's own runs rather than decided
while painting the window, since the row is built by whoever draws the list and the window
is resolved by the kernel.

The pane's pre-existing row rendering SHALL be retained as a test oracle and asserted
buffer-equal — glyph, colour and modifier — at a width that fits and a width that truncates
a directory name, so the convergence is evidenced rather than claimed.

This change SHALL hand no pane over: every row of the code review's handover gate keeps
its verdict, and the row that names this list stays blocked while two kernel surfaces
contest its column.

#### Scenario: The list scrolls by the shared rule

- **WHEN** the changed-files list holds more files than its column has rows and the diff's
  cursor is deep in the list
- **THEN** the window opens the kernel's margin above the current file and clamps at the
  list's tail, rather than pinning that file to the last visible row

#### Scenario: The unscrolled frame does not move

- **WHEN** the list fits in its column
- **THEN** the painted buffer equals what the pane's spans painted, cell for cell,
  including a directory name cut short by a narrow column

#### Scenario: A directory header is not clickable

- **WHEN** the painter reports one row per list child and a directory header is one of them
- **THEN** that row yields no hitbox and the file rows keep the indices they had, so a
  click still jumps the diff to the file it names

#### Scenario: The gate is unmoved

- **WHEN** the convergence lands
- **THEN** the code review's handover gate records the same five outstanding rows, and the
  change states that it hands nothing over
