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

