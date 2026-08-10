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

The catalog SHALL provide text as its only content node, and rows, columns,
lists, dividers, and spacers as its layout nodes. Rows lay children out
horizontally, columns and lists vertically.

#### Scenario: Nested layout

- **WHEN** a plugin returns a column containing rows of text
- **THEN** the tree converts, preserving the nesting and child order

#### Scenario: An empty container

- **WHEN** a plugin returns a column with no children
- **THEN** the tree is valid and renders as empty space

### Requirement: Styling is by theme token, never by color

A node SHALL style itself with a named token drawn from a closed set that
resolves against the active thurbox theme. A plugin MUST NOT be able to specify
a color directly, so that every plugin follows a theme change and none can
render text that is unreadable on the active palette.

#### Scenario: A node carries a known token

- **WHEN** a text node declares a defined style token
- **THEN** the tree converts and the node carries that token

#### Scenario: A node carries an unknown token

- **WHEN** a text node declares a token the host does not define
- **THEN** the result is rejected as invalid, naming the token

#### Scenario: A node carries no token

- **WHEN** a text node declares no style
- **THEN** it renders in the theme's default foreground

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

