## Purpose

Defines the declarative structure a plugin returns to describe what its pane
should show — a closed catalog of layout and content nodes, styled by theme
token rather than by color, so the kernel can render any plugin's output
without running plugin code during a frame.

## ADDED Requirements

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
