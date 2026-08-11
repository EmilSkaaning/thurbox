# Kernel state

## RENAMED Requirements

- FROM: `### Requirement: The open review's diff lines are a published section`
- TO: `### Requirement: The open review's rows are a published section`

## MODIFIED Requirements

### Requirement: The open review's rows are a published section

The published pane snapshot SHALL carry a **review** section describing the rows
the code-review view currently has open, so a pane may draw the review's document
without reading a repository.

The section MUST carry the review's rows **in the order the native pane lists
them**, and each row MUST say which kind it is. The order is not derivable from the
diff: a reviewed file collapses to its header alone, a comment is interleaved after
the line it anchors to, and the summary section follows every file. A pane
recomputing that order would be recomputing kernel view state from a projection of
it.

The kinds are a closed list — a file header, a hunk header, a diff line, a comment,
the summary header, and an informational line — and each MUST carry only what the
pane cannot derive:

- a **file** row: its path, its status as a stable wire name, its insertion and
  deletion counts, whether it is folded, and whether it is marked reviewed;
- a **hunk** row: its old and new starts, its old and new spans, its heading, and
  whether it is marked reviewed. The spans MUST be computed over the hunk's whole
  line list for the gutter width's reason — a bounded window of rows does not
  contain them;
- a **line** row: the path of the file it belongs to, its number on the old side and
  its number on the new side where each exists, whether it is an addition, a
  deletion or context, and its text;
- a **comment** row: its classification as a stable wire name, the first line of its
  body, and whether the body has further lines;
- an **informational** row: its text, which the kernel authors.

The section MUST also carry which row the cursor is on and the width the gutter's
number columns are drawn at.

The gutter width MUST be published rather than left to the pane to derive: it is
computed over **every** hunk of **every** file in the review, and a bounded window
of rows does not contain the largest line number — a pane deriving it from what it
received would draw a narrower gutter than the review's own.

The section MUST NOT carry the pane's rendering. In particular it MUST NOT carry
the line's text already split into syntax-highlighted runs, already windowed to a
horizontal scroll offset, or already padded to a width; and it MUST NOT carry a
row's glyphs — the fold chevron, the file-status glyph, the reviewed mark, the diff
sign and the classification badge's brackets are all the pane's, published as the
facts they are drawn from. How a diff body is coloured is likewise the pane's
decision, and a pane arranging runs the kernel coloured would be evidence about
nothing.

It MUST convey no power over a repository: no diff may be requested, no revision
range named, no file read, and no `git` invoked. The section is built from the
review the **user** already opened, exactly as the file section is built from the
tree the file viewer already opened, and it is empty until then.

#### Scenario: A pane reads the open diff

- **WHEN** a review is open and a plugin holding the capability reads the section
- **THEN** it receives one entry per published row, in the order the native pane
  lists them, each tagged with its kind and carrying that kind's facts

#### Scenario: A folded file is one row

- **WHEN** a file is folded because it has been marked reviewed
- **THEN** the section carries that file's header row and none of its hunks or
  lines, so the pane draws the fold without deciding it

#### Scenario: Nothing is open

- **WHEN** no review has been opened for the active session
- **THEN** the section is present and empty, and a pane reading it draws its
  empty state rather than failing

#### Scenario: The gutter width comes from the whole review

- **WHEN** the published window holds only lines with small numbers while the
  review contains a four-digit line number
- **THEN** the published gutter width is the one the native pane uses, not the one
  the window implies

#### Scenario: A hunk's spans come from its whole line list

- **WHEN** a hunk's lines are cut short by the section's bound
- **THEN** the published spans are still the hunk's own, so the `@@` ranges the
  pane draws are the review's

#### Scenario: No repository power is conferred

- **WHEN** the capability's bindings are enumerated
- **THEN** there is none that lists a revision, reads a file, produces a diff, or
  runs a version-control command

### Requirement: The review section is bounded, and the bound is the node budget

The published review section SHALL be bounded to a maximum number of rows, and the
bound MUST be applied before any per-row work.

The bound counts **every** kind of row, not only diff lines: a header, a comment
and a line all cost nodes, and a bound that counted one kind would be a bound on a
fraction of the tree.

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

- **WHEN** the open review holds more rows than the bound
- **THEN** the section carries exactly the bound's worth of rows

#### Scenario: A cursor past the cap is dropped

- **WHEN** the cursor's row falls outside the published rows
- **THEN** the section names no cursor

#### Scenario: The feature is off

- **WHEN** the code-review feature is disabled
- **THEN** the section is empty whatever the session's state

## ADDED Requirements

### Requirement: A row whose text names a keybinding is published as text

When a native row's text names a **kernel keystroke**, that text SHALL cross in the
snapshot rather than being composed by the pane.

The rule the snapshot otherwise follows is that a rendering does not cross: the
pane composes its own strings from published facts. A string naming a key is the
exception, and for a reason a pane cannot argue with — a plugin pane does not
receive that key, so a pane composing the string would be advertising an action it
cannot perform, and a pane omitting the hint would draw a different row from the one
it reproduces. Only the kernel can honestly author it.

The exception MUST stay narrow: it licenses publishing the text of a row that names
a key, not publishing a row's rendering generally.

#### Scenario: The summary header crosses whole

- **WHEN** a pane draws the review's summary header, whose native text names the key
  that adds a summary comment
- **THEN** the label crosses as text in the snapshot, and the pane draws it
  unchanged

#### Scenario: A row naming no key composes in the pane

- **WHEN** a row's text is composed only from facts — a hunk's ranges, a file's
  counts
- **THEN** the facts cross and the pane composes the string
