# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: The session list is a published snapshot section

The published pane snapshot SHALL carry a **session-list** section describing
every row the session list renders, in the order it renders them, so a pane may
draw the session list without reading the running sessions themselves.

Each row MUST carry the session's name; its status in drawable form (the same
name, label, glyph and style-token quadruple the active-session reader
publishes); the repo-group label when the row is the first of its group and
nothing when it is not; how deeply the row is nested under its parent; whether
its parent renders in a different group; whether the session runs on a remote
host; whether it has a worktree; whether the user's cursor is on it; whether a
running search dimmed it; the byte offsets that search matched in its name; and
the activity and notification text its agent last reported.

The view facts among those — the cursor, the dimming, the matched offsets, the
group label, the nesting depth — MUST be resolved by the kernel, because the
kernel owns the keyboard that moves the cursor, the search that dims, and the
ordering and grouping rules that decide which row opens a group and which row is
a child of which.

The section MUST NOT carry the pane's rendering. In particular it MUST NOT carry
a row's composed line, its glyph padded to a width, its prefix marks, the frames
of the working animation, or a single resolved status text chosen between the
activity and the notification: which of those a row shows, and what each looks
like, is the pane's decision.

It MUST NOT carry any row's text already fitted to a column. The fit is computed
against a resolved pane width, a plugin's pane is a different rect from the
native one, and a row fitted to another pane's width is wrong at its own.

#### Scenario: A pane reads the session list

- **WHEN** sessions are running and a plugin holding the capability reads the
  section
- **THEN** it receives one entry per rendered row, in the pane's own order, each
  with its name, its status quadruple, its group label or none, its depth, its
  flags, its matched offsets and its reported text

#### Scenario: No sessions are running

- **WHEN** no session exists
- **THEN** the section is an empty list rather than absent, so a pane draws its
  own empty state instead of branching on a missing section

#### Scenario: The cursor is hidden

- **WHEN** the interface is showing a context in which the active session is
  irrelevant, so no row is highlighted
- **THEN** no row reports that the cursor is on it

#### Scenario: A row is not pre-fitted

- **WHEN** a session's name or reported activity text is longer than the native
  pane's column
- **THEN** the published row still carries the whole text, and the pane that draws
  it decides what to do with the width it was given

### Requirement: The published session-list section is bounded

The session-list section SHALL carry at most a fixed maximum number of rows, and
the bound MUST be on the **section** rather than on any one consumer.

The bound exists because the cost it prevents is the consumer's: a view tree is
capped at a maximum node count, a session row costs several nodes, and an
unbounded list would make every render of a session-list pane fail rather than
merely scroll. No pane can show that many rows on any terminal, so the bound
costs nothing visible.

#### Scenario: More sessions exist than the bound permits

- **WHEN** the number of rendered rows exceeds the bound
- **THEN** the published section carries the first rows up to the bound and no
  more, and a pane reading it renders successfully

### Requirement: The session-list section counts as a kernel-state reader

A plugin declaring the capability that reads sessions SHALL cause the host to
report that kernel state has a reader, so that the publisher builds and publishes
a snapshot at all.

#### Scenario: The only plugin reads sessions

- **WHEN** the only running plugin declares the sessions capability
- **THEN** the host reports that kernel state has a reader and the session-list
  section is published
