# plugin-host/view-tree delta

## ADDED Requirements

### Requirement: A centred line is placed by the kernel

The catalog SHALL provide a node whose children are laid out left to right on **one**
terminal row, with that row placed centrally in the width the node is given. A plugin
MUST NOT be told that width, and MUST NOT be able to specify a column, an offset or a
padding: it declares that the row is centred and the kernel resolves where that is.

The placement MUST use the same centring thurbox's own panes use, so a pane drawing a
centred row through the tree and a native pane drawing one directly cannot land in
different columns.

The node SHALL admit exactly the children an inline line admits — a text run, a fill, a
nested line, or a motion whose every frame is itself inlineable — and MUST refuse any
other kind at conversion, naming the offending kind. Its content MUST be clipped at one
row rather than wrapped, so a centred row cannot push its siblings down.

The node itself MUST NOT be admissible inside a line, because its width comes from the
area it is given rather than from its content.

#### Scenario: A centred row sits in the middle of its width

- **WHEN** a plugin returns a centred node holding one run, in an area wider than the run
- **THEN** the run is drawn with the leftover columns divided between its two sides

#### Scenario: The odd column matches the kernel's own centring

- **WHEN** the leftover width is an odd number of columns
- **THEN** the extra column falls on the same side thurbox's own centred panes leave it

#### Scenario: A centred row that fills its width

- **WHEN** a centred node's runs are exactly as wide as the area
- **THEN** it renders exactly as an uncentred line of the same runs would

#### Scenario: A centred row longer than its width is clipped

- **WHEN** a centred node's runs total more columns than the area has
- **THEN** the row is clipped at one row, and a following sibling is drawn on the next row

#### Scenario: Several styled runs stay one row

- **WHEN** a centred node holds runs in different styles
- **THEN** they pack left to right at their own widths, each in its own style, and the
  group is centred as one

#### Scenario: A child with no intrinsic width is refused

- **WHEN** a centred node holds a column, a list, a gauge or a spacer
- **THEN** conversion fails naming the offending kind, rather than measuring it as zero

#### Scenario: A centred node inside a line is refused

- **WHEN** a line holds a centred node
- **THEN** conversion fails naming the centred node's kind

#### Scenario: A centred node names itself

- **WHEN** the host is asked for the node's kind name
- **THEN** it reports its own wire name, like every other kind in the catalog

### Requirement: The centred-line constructor is part of the granted module surface

The module a plugin requires SHALL build a centred line through a constructor, rather than
by spelling a node table by hand, and the declared type surface MUST describe it —
otherwise a strict type-check of a bundled pane would reject the call.

The constructor MUST grant no capability: it adds a node kind to the drawing vocabulary
and changes nothing a plugin may read, write, run or reach.

#### Scenario: A plugin builds a centred line through the constructor

- **WHEN** a plugin calls the granted constructor with a table of runs
- **THEN** the resulting node is a centred line carrying those runs

#### Scenario: The constructor takes no children

- **WHEN** a plugin calls it with no arguments
- **THEN** the node is valid and its row renders blank, like an empty line

#### Scenario: The vocabulary grants nothing

- **WHEN** the host's capability set and module bindings are compared before and after
- **THEN** neither has changed
