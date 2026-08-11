# plugin-host/view-tree Specification

## Purpose
Defines the declarative structure a plugin returns to describe what its pane
should show — a closed catalog of layout and content nodes, styled by theme
token rather than by color, so the kernel can render any plugin's output
without running plugin code during a frame.
## Requirements
### Requirement: The node catalog is closed

The view tree SHALL consist only of node kinds the host defines. A tree
containing an unrecognized node kind MUST be rejected as invalid rather than
rendered partially or with the unknown node skipped.

#### Scenario: A plugin returns an unknown node kind

- **WHEN** a plugin's render result contains a node whose kind the host does
  not define
- **THEN** the result is rejected as invalid, naming the unrecognized kind

#### Scenario: A plugin returns only known kinds

- **WHEN** every node in a render result is a defined kind
- **THEN** the tree converts successfully

### Requirement: Content and layout nodes

The catalog SHALL provide text as its only free-form content node, and rows,
columns, lists, dividers, and spacers as its layout nodes. Rows lay children out
horizontally, columns and lists vertically. The catalog SHALL additionally
provide two nodes whose rendering the kernel resolves from the area it is given
rather than from content alone: a **gauge**, and a **paragraph** that wraps.
A node kind not in the catalog MUST be rejected.

#### Scenario: Nested layout

- **WHEN** a plugin returns a column containing rows of text
- **THEN** the tree converts, preserving the nesting and child order

#### Scenario: An empty container

- **WHEN** a plugin returns a column with no children
- **THEN** the tree is valid and renders as empty space

#### Scenario: Every catalog kind names itself

- **WHEN** the host is asked for a node's kind name
- **THEN** each of text, row, line, column, list, divider, spacer, motion,
  gauge and paragraph reports its own wire name

### Requirement: Styling is by theme token, never by color

A node SHALL style itself with a named token drawn from a closed set that
resolves against the active thurbox theme. A plugin MUST NOT be able to specify
a color directly, so that every plugin follows a theme change and none can
render text that is unreadable on the active palette.

The token set SHALL cover every distinct palette role thurbox's own panes draw
with, so that a pane rendered through the view tree needs no colour outside the
set. In particular the set SHALL include one token per session status, because a
status indicator is the one case where the token *is* the meaning and an
approximate colour is wrong rather than merely different.

Every token MUST resolve to a colour distinguishable from the palette's
background on every built-in palette, light and dark alike.

#### Scenario: A node carries a known token

- **WHEN** a text node declares a defined style token
- **THEN** the tree converts and the node carries that token

#### Scenario: A node carries an unknown token

- **WHEN** a text node declares a token the host does not define
- **THEN** the result is rejected as invalid, naming the token

#### Scenario: A node carries no token

- **WHEN** a text node declares no style
- **THEN** it renders in the theme's default foreground

#### Scenario: Every session status has its own token

- **WHEN** a pane draws an indicator for each session status the kernel defines
- **THEN** a distinct token exists for each, and each resolves to that status's
  own palette colour rather than to a neighbouring role's

#### Scenario: No token is invisible on any built-in palette

- **WHEN** each token is resolved against each built-in palette
- **THEN** none resolves to that palette's background colour

### Requirement: Trees are bounded

The host SHALL bound a view tree's depth and total node count, and MUST reject
a tree that exceeds either. A plugin MUST NOT be able to exhaust host memory or
stack by returning a pathological tree.

#### Scenario: A tree nested past the depth limit

- **WHEN** a plugin returns a tree deeper than the limit
- **THEN** the result is rejected as invalid rather than converted

#### Scenario: A tree with more nodes than the limit

- **WHEN** a plugin returns a tree with more nodes than the limit
- **THEN** the result is rejected as invalid

#### Scenario: A tree within both limits

- **WHEN** a tree is within the depth and node bounds
- **THEN** it converts successfully

### Requirement: Text content is bounded and sanitized

A text node's content SHALL be truncated to a bounded length, and control
characters that would corrupt the terminal — escape sequences in particular —
MUST NOT reach the screen.

#### Scenario: A plugin emits an escape sequence

- **WHEN** a text node's content contains an ANSI escape sequence
- **THEN** the sequence does not reach the terminal as a control code

#### Scenario: A plugin emits a very long string

- **WHEN** a text node's content exceeds the length bound
- **THEN** the content is truncated rather than rejected

### Requirement: Conversion never panics

Converting a plugin's render result SHALL return an error for any malformed
input rather than panicking. No value a plugin can construct — wrong types,
cycles, missing fields, deeply nested tables — may crash the host.

#### Scenario: The result is not a table

- **WHEN** a plugin's render returns a number
- **THEN** conversion fails with an error naming what was expected

#### Scenario: A node is missing a required field

- **WHEN** a node omits a field its kind requires
- **THEN** conversion fails naming the node kind and the missing field

#### Scenario: A self-referential structure

- **WHEN** a plugin returns a table that contains itself
- **THEN** conversion fails via the depth bound rather than looping forever

### Requirement: A line composes differently-styled runs at intrinsic width

The catalog SHALL provide an inline line node whose children are laid out left
to right on one terminal row, each occupying exactly the width its own content
needs. A line MUST NOT divide its area into equal shares, and a line whose
content exceeds the available width MUST be clipped at the pane edge rather than
wrapped onto further rows.

#### Scenario: Two runs with different styles share one row

- **WHEN** a plugin returns a line containing a muted run and an accented run
- **THEN** both render on the same terminal row, each in its own style, with the
  second starting immediately after the first ends

#### Scenario: A short run does not get an equal share

- **WHEN** a plugin returns a line whose first run is one character and whose
  second is twenty, in an area wider than the two combined
- **THEN** the first run occupies one column and the second begins at the second
  column

#### Scenario: A line longer than the pane is clipped

- **WHEN** a plugin returns a line whose runs total more columns than the pane
  has
- **THEN** the visible portion renders on one row and the remainder is dropped,
  leaving following siblings on their own rows

#### Scenario: An empty line occupies its row and draws nothing

- **WHEN** a plugin returns a line with no children
- **THEN** the tree is valid and the row renders blank

### Requirement: Only nodes with an intrinsic width may appear in a line

A line SHALL accept only children whose width is determined by their own
content: a text run, a motion, or a nested line. A child of any other kind MUST
be rejected at conversion, naming the offending kind, rather than being
measured, skipped, or drawn. The restriction MUST apply recursively, so a
motion inside a line whose frames are not themselves inlineable is rejected
too.

#### Scenario: A column inside a line

- **WHEN** a plugin returns a line containing a column
- **THEN** conversion fails, naming the kind that cannot be laid out inline

#### Scenario: A divider or spacer inside a line

- **WHEN** a plugin returns a line containing a divider or a spacer
- **THEN** conversion fails rather than the node being silently dropped

#### Scenario: A motion whose frames are inlineable

- **WHEN** a plugin returns a line containing a motion whose every frame is a
  text run
- **THEN** the tree converts and the motion renders inline among the other runs

#### Scenario: A motion smuggling a column into a line through a frame

- **WHEN** a plugin returns a line containing a motion one of whose frames is a
  column
- **THEN** conversion fails, so the restriction cannot be evaded through a
  frame

#### Scenario: A nested line

- **WHEN** a plugin returns a line containing another line of text runs
- **THEN** the tree converts and every run renders on the one row, in order

### Requirement: A motion in a line reserves its widest frame

A motion laid out inside a line SHALL occupy the width of its widest frame for
as long as it is drawn, and a frame narrower than that MUST be padded rather
than allowed to shorten the line. Runs following a motion MUST therefore stay at
a fixed column while the animation runs.

#### Scenario: Frames of unequal width

- **WHEN** a motion in a line has frames of one, three and five columns and the
  host draws the one-column frame
- **THEN** the run following the motion begins five columns after the motion
  starts

#### Scenario: The following run does not move between frames

- **WHEN** the host advances such a motion from one frame to another
- **THEN** the column at which the following run begins is unchanged

### Requirement: A line's runs count against the tree bounds

A line's children SHALL be ordinary nodes for the purpose of the host's node
count and depth limits. A line MUST NOT provide a way to carry content that
escapes those bounds.

#### Scenario: Runs are counted

- **WHEN** a line's runs would take the tree past the host's node bound
- **THEN** the tree is rejected exactly as any other oversized tree is

### Requirement: The line constructor is part of the granted module surface

The host SHALL expose a line constructor in the same frozen constructor table as
the other node kinds, so a plugin never writes the kind string by hand, and the
published type declarations MUST declare it.

#### Scenario: The constructor is present without any capability

- **WHEN** a plugin with no declared capabilities reads the constructor table
- **THEN** the line constructor is present, since building a node grants no host
  power

### Requirement: A gauge is drawn by the kernel from a label, a percentage and an optional suffix

The catalog SHALL provide a gauge node carrying a label, a percentage and an
optional suffix, and the kernel SHALL resolve its geometry from the area the
gauge is given. A plugin MUST NOT need — and MUST NOT be given — the resolved
width of its pane in order to draw one.

A gauge SHALL render as a header followed by a bar row: the header carries the
label at the left and the suffix (or the percentage, when no suffix is given)
flush right, and the bar fills the full width, proportionally filled. Where the
label and the right-hand text together exceed the available width the header
SHALL wrap onto further rows and the bar MUST move down with it, rather than
being drawn over the overflow or the overflow being dropped. A percentage
outside 0–100 MUST be clamped rather than rejected, so a metric that momentarily
overshoots draws a full bar instead of failing the pane.

A gauge SHALL NOT be admissible inside a line, since its width comes from its
area rather than its content.

#### Scenario: The suffix is flush right

- **WHEN** a gauge with a label and a suffix is drawn into an area
- **THEN** the label begins at the first column and the suffix ends at the last

#### Scenario: No suffix falls back to the percentage

- **WHEN** a gauge carries no suffix
- **THEN** the header's right-hand text is the percentage, rounded to a whole
  number and followed by a percent sign

#### Scenario: The bar fills proportionally across the whole width

- **WHEN** a gauge at 0%, 50% and 100% is drawn into the same area
- **THEN** the bar row spans the full width in each case, and the filled portion
  is none, about half, and all of it respectively

#### Scenario: An out-of-range percentage is clamped

- **WHEN** a gauge is given a percentage above 100 or below 0
- **THEN** it draws as 100% or 0% respectively, and the tree is not rejected

#### Scenario: A gauge inside a line

- **WHEN** a plugin returns a line containing a gauge
- **THEN** conversion fails, naming the kind that cannot be laid out inline

#### Scenario: A gauge whose header fits is two rows tall

- **WHEN** a gauge whose label and suffix fit the width is one child of a list
- **THEN** the following sibling begins two rows below the gauge

#### Scenario: A gauge whose header overflows grows

- **WHEN** a gauge's label and right-hand text together exceed the width
- **THEN** the header wraps, the bar is drawn on the row after the last header
  row, and the following sibling begins after the bar

### Requirement: A paragraph wraps its runs onto as many rows as it needs

The catalog SHALL provide a paragraph node whose children are inline runs, laid
out left to right and soft-wrapped onto further rows when they exceed the
available width. A paragraph's height SHALL therefore be a function of the width
it is given, and the host MUST allocate it that many rows so that a following
sibling is not overdrawn.

A paragraph SHALL accept the same children a line does — those whose width comes
from their own content — and MUST reject any other kind, naming it. A paragraph
SHALL NOT itself be admissible inside a line, since its height is not one row.

Where a line clips, a paragraph wraps: both exist because agent-supplied text of
unbounded length must remain readable, while a row of fixed-width fields must
not push its neighbours down.

#### Scenario: Content wider than the pane wraps rather than clipping

- **WHEN** a paragraph whose runs total more columns than the pane has is drawn
- **THEN** the overflow appears on the following row rather than being dropped

#### Scenario: A following sibling is not overdrawn

- **WHEN** a paragraph that wraps onto three rows is followed by a text node in
  a list
- **THEN** the text node renders on the fourth row

#### Scenario: Content narrower than the pane occupies one row

- **WHEN** a paragraph's runs fit the available width
- **THEN** it occupies exactly one row

#### Scenario: Runs keep their own styles across the wrap

- **WHEN** a paragraph holds a muted label run followed by a long unstyled value
  run that wraps
- **THEN** the label keeps its own style and the wrapped remainder keeps the
  value's

#### Scenario: A paragraph is truncated by the bottom of its area

- **WHEN** a paragraph that wants three rows is given two
- **THEN** its first two rows render and the third is dropped rather than
  overflowing the area

#### Scenario: A column inside a paragraph

- **WHEN** a plugin returns a paragraph containing a column
- **THEN** conversion fails, naming the kind that cannot be laid out inline

### Requirement: A tree's identity survives a value that is not directly comparable

A view tree SHALL remain comparable and hashable as a whole, so that an
identical re-push is recognised as unchanged and a motion keeps its phase. A
node carrying a value with no total equality — a gauge's percentage — MUST
therefore compare by its exact representation rather than making the tree
incomparable.

#### Scenario: Two identical gauges compare equal

- **WHEN** two trees each hold a gauge with the same label, percentage and
  suffix
- **THEN** the trees compare equal, so a re-render does not restart an animation
  elsewhere in the tree

#### Scenario: Two gauges differing only in percentage compare unequal

- **WHEN** two trees hold gauges identical but for their percentage
- **THEN** the trees compare unequal

### Requirement: The gauge and paragraph constructors are part of the granted module surface

The host SHALL expose gauge and paragraph constructors in the same frozen
constructor table as the other node kinds, and the published type declarations
MUST declare them. A widening that thurbox's own panes can use but a plugin
cannot would not close the gap it was made for.

#### Scenario: The constructors are present without any capability

- **WHEN** a plugin with no declared capabilities reads the constructor table
- **THEN** the gauge and paragraph constructors are present, since building a
  node grants no host power

#### Scenario: A gauge with a non-finite percentage is refused

- **WHEN** a plugin constructs a gauge whose percentage is not a finite number
- **THEN** conversion fails naming the field, rather than the host drawing an
  undefined bar

### Requirement: A text run may declare emphasis

A text run SHALL be able to declare emphasis independently of its colour token:
bold, dimmed, and underlined. Each MUST be optional and default to off, and each
MUST be applied by the renderer as a text attribute over whatever colour the
run's token resolves to — so emphasis never names a colour and a theme switch
still chooses every colour in the pane.

The three exist because a selectable row needs three distinct appearances that a
colour token alone cannot express: the selected row, a row a running search
filtered out, and the characters that search matched. A list pane cannot be
described by this catalog without them.

#### Scenario: A run declares dim emphasis

- **WHEN** a plugin returns a text run declaring dim
- **THEN** it renders in its token's colour with the terminal's dim attribute,
  and a run that declares none renders without it

#### Scenario: A run declares underline emphasis

- **WHEN** a plugin returns a text run declaring underline
- **THEN** it renders underlined in its token's colour

#### Scenario: Emphasis combines

- **WHEN** a run declares more than one emphasis
- **THEN** every declared attribute is applied to that run and to no other run
  on the line

#### Scenario: Emphasis is not a colour

- **WHEN** a run declares emphasis with no style token
- **THEN** it renders in the theme's default foreground with the emphasis
  applied, and the tree still admits no way to name a colour

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

### Requirement: A run may declare that it belongs to the selected row

A text run SHALL be able to declare that it is part of the row the user's cursor
is on. The host MUST resolve that declaration to the active theme's selection
foreground and selection background, so the plugin names a **role** and the theme
owns both colours — the tree still admits no way to name a colour.

Unlike the emphasis attributes, this declaration MUST **replace** the colour the
run's style token would have resolved to, because a selection is a whole
appearance rather than an attribute applied over one. It MUST compose with the
emphasis attributes, so a selected run can also be bold.

It is a separate declaration from a list's selected row on purpose: thurbox's own
list panes do not agree on what a selected row looks like, so an appearance
inferred from the list's cursor would make at least one of them unreproducible.

#### Scenario: A selected run takes the theme's selection pair

- **WHEN** a plugin returns a text run declaring it belongs to the selected row
- **THEN** it renders in the theme's selection foreground on its selection
  background

#### Scenario: The declaration overrides the run's token

- **WHEN** a selected run also names a style token
- **THEN** the selection colours win, and the same run without the declaration
  renders in its token's colour

#### Scenario: Selection composes with emphasis

- **WHEN** a selected run also declares bold
- **THEN** it renders bold in the selection pair, and a neighbouring run on the
  same line keeps neither the selection nor the emphasis

### Requirement: A run may declare that its row is an insertion or a deletion

A text run SHALL be able to declare that the row it is on is a diff **insertion**
or a diff **deletion**. The host MUST resolve that declaration to the active
theme's added-row and removed-row backgrounds, so the plugin names a **role** and
the theme owns the colour — the tree still admits no way to name a colour.

The declaration MUST be one of exactly two values. An unrecognised value MUST be
refused as a malformed node naming the field and the values that exist, never
ignored, because a silently dropped tint draws a deletion as context.

Like a selected run and unlike the emphasis attributes, this declaration MUST
affect the run's **background** and leave its style token to choose the
foreground: a diff body's colours belong to the pane, and the tint is the only
thing that says which side of the change the row is on.

A run that declares both a tint and that it belongs to the selected row MUST draw
in the selection's background. The cursor's row is one appearance whatever the row
contains, and two backgrounds on one row is not a state the theme defines.

#### Scenario: A tinted run takes the theme's diff background

- **WHEN** a plugin returns a text run declaring its row is an insertion
- **THEN** it renders on the theme's added-row background, and a run declaring a
  deletion renders on the removed-row background

#### Scenario: A tint leaves the foreground to the token

- **WHEN** a tinted run also names a style token
- **THEN** it renders in that token's colour on the tint's background

#### Scenario: Selection wins over a tint

- **WHEN** a run declares both a tint and that it belongs to the selected row
- **THEN** it renders in the theme's selection pair and the tint is not drawn

#### Scenario: An unknown tint is refused

- **WHEN** a plugin declares a tint that is neither of the two the host defines
- **THEN** conversion fails naming the field and the accepted values, and the pane
  reports the error rather than drawing an untinted row

### Requirement: A run may be a fill that consumes a line's remaining width

An inline **fill** run SHALL be available, drawing one repeated glyph across
whatever width is left on its line after every other run has taken its own
intrinsic width.

The width it resolves to MUST be computed by the **host**, at the moment of
drawing, from the area the line was given. A plugin MUST NOT be told that width:
the node exists precisely so that a pane can reach its own right edge without
learning a dimension, which is the trade the gauge node already made for a bar and
the list node made for a scroll window.

A fill MUST be admissible only where inline runs are, and a fill on a line with no
room left MUST draw nothing rather than overflow onto the row below.

Its glyph MUST be a single displayable character, and its style MUST be the same
style any other run may carry — so a fill can carry a tint, which is what makes a
diff row's background reach the pane's edge.

#### Scenario: A fill reaches the line's right edge

- **WHEN** a line holds a text run and then a fill
- **THEN** the drawn row is the text followed by the fill's glyph repeated to the
  line's last column

#### Scenario: A fill carries its style

- **WHEN** a fill declares a tint
- **THEN** the columns it fills are drawn on that tint's background

#### Scenario: A full line leaves a fill nothing

- **WHEN** the runs before a fill already occupy the whole width
- **THEN** the fill draws nothing and no row below it is disturbed

#### Scenario: A fill is refused where inline runs are

- **WHEN** a plugin puts a fill somewhere a text run may not go
- **THEN** conversion fails naming the node kind, as it does for any other
  non-inline child of an inline container

### Requirement: The palette's bright accent is addressable by a token

The closed vocabulary of style tokens SHALL include the palette's **bright
accent**, resolving 1:1 onto that field as every other token resolves onto its
own.

It exists because thurbox's own diff highlighter draws a capitalised type name in
that colour and it is the one colour of the six it uses that no token could name;
approximating it with the ordinary accent would make a pane that highlights code
unreproducible, and the two are separate palette fields a custom theme may set
independently.

#### Scenario: The bright accent resolves to its palette field

- **WHEN** a run names the bright-accent token
- **THEN** it renders in the palette's bright accent, and not in the accent

### Requirement: A text style may be given as a table

The text-run constructor SHALL accept its style either as a token name followed by
positional flags, or as a **single table** naming the token and any of the
emphases, the selection role and the tint.

The positional form MUST keep working unchanged, argument for argument, so no
plugin already written against it is affected. The table form exists because the
positional form was full: a style now carries more fields than a call can
reasonably order, and one long signature growing without limit is worse than two
spellings of which only one can grow.

Both forms MUST produce the same node, so a pane's appearance cannot depend on how
its style was spelled.

#### Scenario: The two forms produce the same node

- **WHEN** the same style is expressed positionally and as a table
- **THEN** the two calls produce identical nodes

#### Scenario: The table form reaches a field the positional form cannot

- **WHEN** a style table names a tint
- **THEN** the node carries it, and the positional form has no argument that could

### Requirement: A list may declare a scroll track

A list node SHALL be able to declare that it wants a scroll track. The
declaration MUST be optional and MUST default to absent, so a list that does not
declare one is laid out exactly as before — the panes that deliberately overflow
without a scrollbar MUST NOT gain one.

When a list declares a track and has more children than the rows it was given,
the **kernel** MUST reserve the rightmost column of the list's area for the
track, draw the thumb there at the declared cursor's position, and lay the rows
out in the width that remains. The column MUST be reserved by the same rule
thurbox's own panes reserve one with, so a native pane and a plugin reproducing
it cannot place the track in different columns or draw different thumbs.

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

### Requirement: The scroll-track declaration is part of the granted module surface

The module a plugin requires SHALL let it declare a track through the same list
constructor it already uses, rather than by spelling a node table by hand, and
the declared type surface MUST describe it — otherwise a strict type-check of a
bundled pane would reject the argument that makes it scroll.

A declaration that is not a boolean MUST be refused as a malformed node naming
the kind and the field, like every other bad field.

#### Scenario: A plugin declares a track through the constructor

- **WHEN** a plugin builds a list through the granted constructor and asks for a
  track
- **THEN** the resulting node carries the declaration and renders with one

#### Scenario: A non-boolean declaration is refused

- **WHEN** a plugin declares a track as a string or a number
- **THEN** conversion fails naming the node kind and the field, and the pane
  reports the error rather than drawing a list

### Requirement: A run may yield its width and be ellipsized by the kernel

A text run SHALL be able to declare that it **yields its width to the other runs on
its line**. The declaration MUST be optional and default to absent, so every line
already written is laid out exactly as before.

When a line holds one or more such runs, the kernel SHALL give every other run its
intrinsic width, hand the remainder to the yielding runs, and truncate them with an
ellipsis when they do not fit. A line with no yielding run MUST clip at the pane's
edge exactly as before.

**Consecutive yielding runs share one budget.** A string split into matched and
unmatched runs is one piece of text to a reader, so the ellipsis MUST fall where the
concatenation would have been cut, and the runs after the cut MUST draw nothing —
never one ellipsis per run.

The truncation SHALL use the **same** fitting the kernel's own panes use, so a
plugin's copy of a pane and that pane cannot disagree about where a title was cut.
The consequence MUST be accepted rather than papered over: that fitting counts
characters, so a run of double-width glyphs can still exceed its budget in cells, as
it does in the kernel's own panes.

A yielding run MUST NOT be given width at the expense of a fill. A fill is the
line's *residue* and a yielding run is bounded by what the fixed runs leave, so the
yielding runs are resolved first and a fill takes whatever remains after them —
which, on a full line, is nothing.

The declaration SHALL be a field of a run's style rather than a new node kind: it
describes how a run is drawn when its line runs out of room, and a node kind would
have to be threaded through every walk over the tree.

#### Scenario: A line that fits is untouched

- **WHEN** a line whose runs fit declares a yielding run
- **THEN** every run draws in full and no ellipsis appears

#### Scenario: A line that overflows

- **WHEN** a line overflows and one of its runs yields its width
- **THEN** the other runs keep their full width and the yielding run is truncated
  with an ellipsis

#### Scenario: A trailing marker survives the overflow

- **WHEN** an overflowing line ends with a fixed run after the yielding one
- **THEN** that run is still drawn, because the yielding run gave up the columns it
  needed

#### Scenario: Several yielding runs

- **WHEN** an overflowing line holds consecutive yielding runs
- **THEN** they are cut as one piece of text, with a single ellipsis at the cut and
  nothing drawn after it

#### Scenario: A line with no yielding run

- **WHEN** an overflowing line declares none
- **THEN** it clips at the pane's edge exactly as before

