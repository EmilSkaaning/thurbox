# plugin-host/view-tree Specification

## MODIFIED Requirements

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

## ADDED Requirements

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
