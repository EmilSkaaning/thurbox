# plugin-host/view-tree Specification

## ADDED Requirements

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
