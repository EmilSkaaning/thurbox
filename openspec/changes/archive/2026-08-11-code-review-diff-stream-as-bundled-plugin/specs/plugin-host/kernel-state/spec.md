# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: The open review's diff lines are a published section

The published pane snapshot SHALL carry a **review** section describing the diff
lines the code-review view currently has open, so a pane may draw a diff stream
without reading a repository.

Each line MUST carry the path of the file it belongs to, its number on the old
side and its number on the new side where each exists, whether it is an addition,
a deletion or context, and its text. The section MUST also carry which row the
cursor is on and the width the gutter's number columns are drawn at.

The gutter width MUST be published rather than left to the pane to derive: it is
computed over **every** hunk of **every** file in the review, and a bounded window
of rows does not contain the largest line number — a pane deriving it from what it
received would draw a narrower gutter than the review's own.

The section MUST NOT carry the pane's rendering. In particular it MUST NOT carry
the line's text already split into syntax-highlighted runs, already windowed to a
horizontal scroll offset, or already padded to a width: how a diff body is
coloured is the pane's decision, and a pane arranging runs the kernel coloured
would be evidence about nothing.

It MUST convey no power over a repository: no diff may be requested, no revision
range named, no file read, and no `git` invoked. The section is built from the
review the **user** already opened, exactly as the file section is built from the
tree the file viewer already opened, and it is empty until then.

#### Scenario: A pane reads the open diff

- **WHEN** a review is open and a plugin holding the capability reads the section
- **THEN** it receives one entry per published diff line, in the order the native
  pane lists them, each with its path, its two line numbers, its kind and its text

#### Scenario: Nothing is open

- **WHEN** no review has been opened for the active session
- **THEN** the section is present and empty, and a pane reading it draws its
  empty state rather than failing

#### Scenario: The gutter width comes from the whole review

- **WHEN** the published window holds only lines with small numbers while the
  review contains a four-digit line number
- **THEN** the published gutter width is the one the native pane uses, not the one
  the window implies

#### Scenario: No repository power is conferred

- **WHEN** the capability's bindings are enumerated
- **THEN** there is none that lists a revision, reads a file, produces a diff, or
  runs a version-control command

### Requirement: The review section is bounded, and the bound is the node budget

The published review section SHALL be bounded to a maximum number of rows, and the
bound MUST be applied before any per-row work.

The bound exists for a different reason than the task, automation and file
sections' bounds, and the difference MUST be recorded rather than absorbed: those
sections bound a row count because a pane draws a bounded number of rows, whereas
this one bounds it because a diff line's **internal** structure is unbounded — one
node per syntax token — and the view tree's node budget is a whole-tree budget.
Every earlier pane could return its whole list; this one cannot.

The cursor MUST be dropped when it falls outside the rows that survived the bound,
so a pane never receives an anchor into rows it was not given.

The section MUST be empty when the code-review feature is disabled, mirroring the
sections that are empty when their own feature is off.

#### Scenario: A large diff is capped

- **WHEN** the open review holds more lines than the bound
- **THEN** the section carries exactly the bound's worth of rows

#### Scenario: A cursor past the cap is dropped

- **WHEN** the cursor's row falls outside the published rows
- **THEN** the section names no cursor

#### Scenario: The feature is off

- **WHEN** the code-review feature is disabled
- **THEN** the section is empty whatever the session's state
