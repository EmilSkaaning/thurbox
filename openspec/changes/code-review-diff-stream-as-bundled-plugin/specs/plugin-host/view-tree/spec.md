# plugin-host/view-tree Specification

## ADDED Requirements

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
