# plugin-host/view-tree Specification

## ADDED Requirements

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
