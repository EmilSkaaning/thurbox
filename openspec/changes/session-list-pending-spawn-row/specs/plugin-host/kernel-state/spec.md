# plugin-host/kernel-state delta

## MODIFIED Requirements

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

The section SHALL also carry the row for a session that **does not exist yet** — a
spawn in flight — at the position it will occupy once it lands, so a pane draws
the interface's only progress surface for a non-blocking new-session flow. Such a
row MUST be distinguishable from a session's row by a fact it carries and a
session's row never does, and MUST carry the short phase label the flow is on.
Where the spawn opens a repo group that has no rows yet, the placeholder MUST
carry that group's label exactly as a real first-of-group row does.

The placeholder's position MUST be resolved by the kernel, by the same rule that
decides where the real row will appear, and MUST NOT be derivable by the pane:
the pane is not told which repos the spawn will span.

A placeholder MUST NOT be reportable as the cursor's row and MUST NOT be
selectable, because there is no session to select.

The view facts among those — the cursor, the dimming, the matched offsets, the
group label, the nesting depth — MUST be resolved by the kernel, because the
kernel owns the keyboard that moves the cursor, the search that dims, and the
ordering and grouping rules that decide which row opens a group and which row is
a child of which.

The section MUST NOT carry the pane's rendering. In particular it MUST NOT carry
a row's composed line, its glyph padded to a width, its prefix marks, the frames
of the working animation, or a single resolved status text chosen between the
activity and the notification: which of those a row shows, and what each looks
like, is the pane's decision. The same holds for a placeholder: whether its
glyph spins, what it spins as, and how its phase label is set beside its name are
the pane's.

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

#### Scenario: A spawn is in flight in a group that already has rows

- **WHEN** a session is being created for a repo the list already shows
- **THEN** the section carries an extra row at the end of that repo's group,
  carrying the phase label and no group label, and a pane can tell it from the
  sessions around it

#### Scenario: A spawn is in flight for a repo with no rows yet

- **WHEN** a session is being created for a repo that has no sessions
- **THEN** the extra row is last in the section and carries that repo's group
  label, so the pane draws a header above it

#### Scenario: The placeholder is not a session

- **WHEN** a spawn is in flight while the cursor is on some session
- **THEN** the placeholder row does not report the cursor, and the row that does
  is still the session the cursor is on

#### Scenario: No spawn is in flight

- **WHEN** nothing is being created
- **THEN** every row in the section is a session's, and none carries a phase
  label
