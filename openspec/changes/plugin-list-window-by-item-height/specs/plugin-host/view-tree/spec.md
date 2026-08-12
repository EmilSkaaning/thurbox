# plugin-host/view-tree delta

## MODIFIED Requirements

### Requirement: A list may carry the row its cursor is on

A list node SHALL be able to declare which of its children the user's cursor is
on. The declaration MUST be optional — a list without one behaves exactly as
before — and MUST be an index into the list's own children, expressed in the
one-based form the plugin's array uses.

When a list declares a selected row and has more children than the rows it was
given, the **kernel** MUST choose which slice of children to draw, keeping the
selected child visible. A plugin MUST NOT be told the height it was given, and it
MUST NOT be required to window its own list: the whole reason this exists is that
a pane whose list cannot scroll to its cursor is not a reproduction of a pane
whose list can.

**A child is one row of the list however many lines it renders as.** A list's
children are its rows for the cursor, for a click and for the window alike, so a
child that stacks several lines — a record with a heading above it, say — MUST be
kept whole: it counts once toward the index a plugin declares and a click reports,
and it counts as its rendered height toward the rows the window fits. The kernel
MUST resolve the window in **rows**, so a list of taller children scrolls to its
cursor instead of clipping it, and MUST NOT require a plugin to know or declare
any child's height.

A list whose children each render as a single line MUST be windowed exactly as it
was before children could be taller: the row-measured rule is a generalisation of
the row-count rule and MUST agree with it everywhere the two are both defined.

The chosen slice MUST be resolved by the same rule thurbox's own panes use, so a
native pane and a plugin reproducing it cannot scroll differently.

An index outside the list's children MUST be refused as a malformed node naming
the field, not clamped — including zero, which is what a plugin passing a
zero-based index would send.

#### Scenario: A list shorter than its area draws every row

- **WHEN** a plugin returns a list declaring a selected row and the pane has room
  for every child
- **THEN** every child is drawn, in order, and the declaration changes nothing
  about the layout

#### Scenario: A list longer than its area scrolls to its selection

- **WHEN** a plugin returns a list of more children than the pane has rows, with a
  selected row past the fold
- **THEN** the drawn slice contains the selected child

#### Scenario: A list with no selection draws from its first child

- **WHEN** a plugin returns a list of more children than the pane has rows and
  declares no selection
- **THEN** drawing starts at the first child and the overflow is clipped

#### Scenario: An out-of-range selection is refused

- **WHEN** a plugin declares a selected index of zero, a negative index, or one
  past its last child
- **THEN** conversion fails naming the node kind and the field, and the pane
  reports the error rather than drawing a different list

#### Scenario: A list of two-line children scrolls to its cursor

- **WHEN** a plugin returns a list whose children each stack two lines, more of
  them than the pane has lines, with a selected child past the fold
- **THEN** the selected child is drawn whole, and the drawn slice holds only as
  many children as fit in the pane's lines

#### Scenario: A taller child is still one index

- **WHEN** a list mixes children of one and of several lines and declares a cursor
- **THEN** the declared index counts children, not lines, and the child at that
  index is the one kept in view

#### Scenario: A list of single-line children is windowed as before

- **WHEN** a list whose children each render as one line is windowed at any
  combination of length, cursor and pane height
- **THEN** the drawn slice is identical to the one the row-count rule chose

#### Scenario: A child taller than the pane is still drawn

- **WHEN** a list's selected child renders as more lines than the pane has
- **THEN** that child is drawn and clipped by the pane's bottom, rather than the
  list drawing nothing

### Requirement: A list may declare a scroll track

A list node SHALL be able to declare that it wants a scroll track. The
declaration MUST be optional and MUST default to absent, so a list that does not
declare one is laid out exactly as before — the panes that deliberately overflow
without a scrollbar MUST NOT gain one.

When a list declares a track and renders more **rows** than the pane has lines,
the **kernel** MUST reserve the rightmost column of the list's area for the
track, draw the thumb there at the declared cursor's position, and lay the rows
out in the width that remains. The column MUST be reserved by the same rule
thurbox's own panes reserve one with, so a native pane and a plugin reproducing
it cannot place the track in different columns or draw different thumbs.

The track MUST describe the same quantity the window resolves: whether the list
overflows, how much there is to scroll through, and where the thumb sits are all
counted in rendered rows, so a list of taller children gets a thumb that matches
what a user is scrolling past. For a list whose children each render as one line
this is the count of children, which is what it always was.

When the list fits the rows it was given, no column MUST be reserved and no thumb
drawn: a track that appeared for a list with nothing to scroll would take a
column of content away for no information.

A list that declares a track and no cursor MUST be drawn with the thumb at its
first position rather than refused, because whether a cursor is published is a
decision of whatever the pane reads and a plugin's node shape must not depend on
it.

A plugin MUST NOT be told the width or the height the track was resolved
against, and MUST NOT be able to place, size or style the track: it declares
that the list scrolls, and the kernel owns where that is shown.

The rows a click resolves to MUST exclude the reserved column, so a click on the
thumb is never delivered as a click on a row.

#### Scenario: A declared track appears when the list overflows

- **WHEN** a plugin returns a list that declares a track and has more children
  than the pane has rows
- **THEN** the pane's rightmost column carries a thumb positioned at the declared
  cursor, and the rows are drawn in the remaining width

#### Scenario: A declared track is absent when everything fits

- **WHEN** a plugin returns a list that declares a track and every child fits
- **THEN** no column is reserved and the rows are drawn at the pane's full width

#### Scenario: A list that declares no track is unchanged

- **WHEN** a plugin returns an overflowing list that declares no track
- **THEN** the rows occupy the pane's full width and no thumb is drawn

#### Scenario: A track without a cursor is drawn, not refused

- **WHEN** a plugin returns an overflowing list that declares a track and no
  selected row
- **THEN** conversion succeeds and the thumb is drawn at its first position

#### Scenario: A click on the track is not a click on a row

- **WHEN** a user clicks the column the track occupies
- **THEN** no row hitbox contains that column, so the click does not select a row

#### Scenario: A track appears for children that overflow only in rows

- **WHEN** a list declares a track and holds fewer children than the pane has
  lines, but they render as more lines than the pane has
- **THEN** the track is reserved and a thumb is drawn
