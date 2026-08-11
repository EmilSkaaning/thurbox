# plugin-host/kernel-state Specification

## Purpose
Defines how a plugin reads state the kernel owns — sessions, host metrics,
scheduled automations — without reaching into the running application. A plugin
VM runs on its own thread with no clock and no filesystem, so the channel has to
answer three questions at once: what a plugin is allowed to see (a capability per
kind of state), what the kernel must resolve on its behalf (anything a sandbox
cannot compute), and what it must *not* resolve (any string the pane displays,
because presentation is the pane's job). It also has to cost nothing when nobody
is reading, or an installed plugin would tax every idle tick.
## Requirements
### Requirement: Kernel state reaches a plugin as a published snapshot

The host SHALL expose kernel state to a plugin as a snapshot published by
whichever binary owns that state, held in one process-wide slot of pure data,
and read by a binding when the plugin calls it. A binding MUST NOT reach into
the running application, and reading one MUST NOT require any plugin code to run
on the thread that draws.

A publication MUST be atomic: a snapshot replaces the previous one whole, so a
reader observes either the old value or the new one and never a mixture of the
two. Each reader answers from the most recently published snapshot at the moment
it is called.

#### Scenario: Nothing has been published

- **WHEN** a plugin calls a state reader in a process where no snapshot has been
  published
- **THEN** the reader returns nothing, rather than failing or blocking

#### Scenario: A published snapshot is readable

- **WHEN** a snapshot is published and a plugin holding the matching capability
  calls its reader
- **THEN** the reader returns that snapshot's values

#### Scenario: A later publication replaces an earlier one

- **WHEN** a snapshot is published twice with different values
- **THEN** a reader called afterwards sees only the second

#### Scenario: A reader never sees a partial snapshot

- **WHEN** a snapshot is published while a plugin is reading one
- **THEN** the reader observes either the whole previous snapshot or the whole
  new one, never a mixture of their sections

### Requirement: Publishing kernel state costs nothing when nobody reads it

The publisher SHALL build no snapshot unless at least one running plugin holds a
capability that can read one, and MUST NOT publish a snapshot equal to the one
already published. Both properties MUST be observable through counters rather
than asserted in prose, so a regression is a failing test and not a judgement.

Publishing MUST NOT mark the interface as needing a repaint: a plugin pane
repaints when the tree it returns changes, and coupling the two would make an
installed plugin repaint the screen on every state change whether or not its
pane is on screen.

#### Scenario: No plugin can read kernel state

- **WHEN** the publisher runs repeatedly and no running plugin holds a
  state-reading capability
- **THEN** no snapshot is built and the build counter does not advance

#### Scenario: A reader exists and the state is unchanged

- **WHEN** the publisher runs repeatedly while the state it describes does not
  change
- **THEN** a snapshot is built but the publish counter advances at most once

#### Scenario: The state changes

- **WHEN** a value inside the snapshot changes and the publisher runs
- **THEN** the publish counter advances

#### Scenario: Publishing does not repaint

- **WHEN** a snapshot is published while nothing else has changed
- **THEN** the interface is not marked as needing a repaint

### Requirement: The snapshot carries what a plugin cannot derive, and no more

The snapshot SHALL resolve, on the plugin's behalf, exactly those values a
sandboxed plugin has no way to compute: anything requiring a clock, a filesystem
path, a lookup across kernel records, or a rendering decision the kernel owns.
Quantities SHALL be carried as numbers, and the plugin SHALL compose every string
it displays.

Concretely, the snapshot MUST carry time-to-event as an already-resolved
duration rather than an absolute instant, a directory's display name rather than
its path, a referenced record's name rather than only its identifier, and — for
each session status — the glyph and the style token the kernel draws it with.

#### Scenario: A countdown is resolved before publication

- **WHEN** the snapshot describes an event with a known absolute time
- **THEN** it carries the remaining duration, so a plugin with no clock can
  render the countdown

#### Scenario: A path becomes a display name

- **WHEN** the snapshot describes a repository or an additional directory
- **THEN** it carries the name a user sees, not a filesystem path

#### Scenario: A reference is resolved to a name

- **WHEN** the described session has a parent session
- **THEN** the snapshot carries the parent's name, falling back to a shortened
  identifier when the parent is no longer present

#### Scenario: A status carries how the kernel draws it

- **WHEN** the snapshot describes a session's status
- **THEN** it carries that status's label, its glyph, and the style token the
  kernel resolves it to, so two panes cannot disagree about either

#### Scenario: Quantities are not pre-formatted

- **WHEN** the snapshot describes a byte count, a token count, a duration, a
  cost or a percentage
- **THEN** it carries the number, and the plugin formats it

### Requirement: Each kind of kernel state is a separate capability

Reading kernel state SHALL be gated per kind of state, not by one blanket grant.
A plugin granted one kind MUST NOT be able to read another, and the binding for a
kind it was not granted MUST be absent from its environment.

The kinds are the running **sessions**, host resource **metrics**, and scheduled
**automations**.

#### Scenario: A plugin declares one kind

- **WHEN** a plugin declares only the session-reading capability
- **THEN** the session reader is present and the metrics and automation readers
  are absent

#### Scenario: A plugin declares none

- **WHEN** a plugin declares no state-reading capability
- **THEN** none of the three readers is present in its environment

#### Scenario: A plugin declares all three

- **WHEN** a plugin declares all three state-reading capabilities
- **THEN** all three readers are present and each returns its own section

### Requirement: A state reader answers about the active session only when there is one

The session reader SHALL describe the session the user is currently on, and MUST
return nothing when there is none — a fresh thurbox with no sessions is the
normal case, not an error.

#### Scenario: No session exists

- **WHEN** a plugin holding the session capability reads state with no session
  open
- **THEN** the reader returns nothing and the plugin can render a placeholder

#### Scenario: The active session changes

- **WHEN** the user moves to a different session and the snapshot is republished
- **THEN** the reader describes the newly active session

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

### Requirement: A published row of a writable record carries that record's id

Each published task row and each published automation row SHALL carry the record's
own id. Without it a pane that may change a record could not name the row it drew,
and would have to match on a title it composed itself — a title the kernel
publishes unfitted and a pane is free to alter.

The id MUST be published whether or not the reading plugin holds a write
capability, because the snapshot is one shape for every reader and a per-grant
snapshot would make what a pane sees depend on what it may do.

Publishing an id MUST NOT be read as publishing a handle: it is the same opaque row
number `thurbox-cli` already prints, it grants nothing on its own, and every change
made with it still requires the capability that permits that change.

#### Scenario: A task row carries its id

- **WHEN** a plugin holding the task capability reads the task list
- **THEN** each entry carries the task's id alongside its title and status

#### Scenario: An automation row carries its id

- **WHEN** a plugin holding the automation capability reads the scheduled
  automations
- **THEN** each entry carries the automation's id

#### Scenario: The id crosses without a write capability

- **WHEN** a plugin declares only the read capability for tasks
- **THEN** the rows still carry their ids, and no mutating binding is present

### Requirement: The automations pane's rows are a published snapshot section

The published snapshot SHALL carry the automations thurbox has, as one section in
the order the automations pane lists them. The section MUST always be present —
"there are none" is knowledge the kernel has — so a pane iterates it without a nil
check, and it MUST carry every automation, enabled and disabled alike, rather than
only those due to fire.

Each row MUST carry what a pane draws it from: the automation's name, whether it is
enabled, its action's stable wire name, the resolved label of its schedule, the
whole seconds until it is next due, whether a running search filtered the row out,
and the byte offsets in the name that the search matched.

The section MUST NOT carry the marker glyph, the colour role, or the emphasis of a
row: which glyph an enabled automation draws as, and in what colour, is the pane's
decision.

This section is distinct from the already-published list of *upcoming* automations,
which is filtered to those that are enabled and scheduled and exists for a
different pane. Both MUST be readable through the same declared capability.

#### Scenario: Every automation crosses, not only the due ones

- **WHEN** a plugin holding the automations capability reads the pane section while
  one automation is enabled and scheduled and another is disabled
- **THEN** it receives both rows, each reporting its own enabled state

#### Scenario: A row carries no rendering

- **WHEN** a plugin reads a row
- **THEN** the row carries its name, enabled flag, action name, schedule label,
  countdown and search verdict, and carries no glyph, colour token or style

#### Scenario: Both automation readers answer from one capability

- **WHEN** a plugin declares the automations capability alone
- **THEN** both the upcoming-automations reader and the pane-section reader are
  present in its environment

### Requirement: A composed display string is published as its parts

When a pane's row shows a string the kernel composes from several facts, the
snapshot SHALL publish those facts and the plugin SHALL compose the string —
except for the parts a sandboxed plugin cannot compute, which the kernel MUST
resolve.

For an automation's summary that means the schedule's **resolved human label** (a
cron expression's meaning is thurbox's own vocabulary) and the **countdown in whole
seconds** (a plugin has no clock) are published, while the separator, the ordering,
and the precedence that shows `disabled` for a disabled automation, a countdown for
a scheduled one, and a placeholder for an enabled one with no next run, are the
plugin's.

The snapshot MUST NOT publish the finished summary string, because a pane assembled
from strings the kernel formatted is not evidence that a pane can own its
presentation.

#### Scenario: The parts cross and the string does not

- **WHEN** a plugin reads a row for an automation scheduled by a cron expression
- **THEN** it receives the schedule's human label, the action's name and a duration
  in seconds, and no field holds the assembled summary

#### Scenario: A disabled automation still reports its schedule

- **WHEN** the automation is disabled
- **THEN** the row still carries its schedule label and action, and the plugin is
  what decides that a disabled row shows `disabled` in place of a countdown

### Requirement: A list section's scroll anchor and drawn cursor are separate facts

A published list section SHALL carry the row a pane scrolls to and whether that row
is **drawn** as the cursor as two separate facts, because a pane may window to its
cursor while not showing one — thurbox's automations pane scrolls to the cursor's
row whether or not it holds focus, and highlights it only when it does or when a
global search is previewing a row there.

The kernel MUST resolve both, since a plugin can observe neither focus nor a search
preview. Where the drawn cursor is a property of the *section* — as it is for a pane
whose whole cursor appears or disappears with its focus — the section MUST publish it
once as a section-level flag rather than restating it on every row, so that a
publication cannot highlight one row while scrolling to another.

The section MUST also carry whether the pane holds focus when that changes what is
drawn, so an empty pane names the key that adds an automation only when the pane
can receive it.

#### Scenario: An unfocused pane still names its anchor

- **WHEN** the pane does not hold focus and the cursor is on a row below the fold
- **THEN** the section names that row as the anchor and reports that the cursor is
  not drawn, so a plugin's list scrolls there without highlighting it

#### Scenario: A search preview draws the cursor without focus

- **WHEN** a global search is previewing an automation while focus is in the search
  strip
- **THEN** the section reports the cursor as drawn

#### Scenario: The focus fact reaches the empty state

- **WHEN** there are no automations and the pane holds focus
- **THEN** the section reports focus, and the plugin's empty-state line is the one
  that names the key

### Requirement: The published automations section is bounded and respects its feature

The number of automation rows published SHALL be bounded, so a large automation
list cannot produce a view tree beyond the node budget — which would make every
render of an automations pane *fail* rather than merely scroll.

When more automations exist than the bound allows, the section MUST publish the
first rows up to the bound and MUST NOT publish an anchor that falls outside them.

The section MUST be empty when the automations feature is disabled, mirroring the
task and file sections: thurbox draws no automations pane and fires no schedules in
that configuration, so a pane advertising them would surface a disabled feature.

#### Scenario: A very long list is truncated rather than failing a render

- **WHEN** there are more automations than the bound
- **THEN** the section carries the bound's worth of rows and a pane built from it
  renders

#### Scenario: An anchor beyond the bound is not published

- **WHEN** the cursor is on a row past the bound
- **THEN** the section publishes no anchor

#### Scenario: The feature is off

- **WHEN** the automations feature is disabled
- **THEN** the section is empty

