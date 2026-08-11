# plugin-host/authoring Specification

## Purpose
Defines the constructor surface a plugin uses to build view nodes, so that a
malformed node is a mistake the author's tooling catches rather than a runtime
rejection the user sees.
## Requirements
### Requirement: The host module provides node constructors

The `@thurbox` module SHALL expose a `ui` table with a constructor for every
node kind the view tree defines. A plugin MUST be able to build any valid tree
without writing a `kind` field itself.

#### Scenario: Building a text node

- **WHEN** a plugin calls the text constructor with content
- **THEN** it receives a node the host converts to a text node

#### Scenario: Building a container

- **WHEN** a plugin calls a container constructor with child nodes
- **THEN** it receives a node the host converts with those children in order

#### Scenario: Every node kind has a constructor

- **WHEN** the `ui` table is inspected
- **THEN** it has a constructor for each kind the view tree defines

### Requirement: Constructors are available without a capability

The `ui` constructors SHALL be present for every plugin regardless of the
capabilities it declared, because they build plain values and grant no host
power.

#### Scenario: A plugin with no capabilities

- **WHEN** a plugin that declared no capabilities inspects the `ui` table
- **THEN** the constructors are present

### Requirement: Styling accepts only defined tokens

A constructor that takes a style SHALL accept the defined style tokens and MUST
reject anything else at conversion, naming the offending token.

A style's non-token fields — the emphases, the selection role, the tint, and the
width-yielding declaration — SHALL be **opt in by exact value**: anything that is not
the boolean true leaves the field off, so a misspelled field cannot silently change
how a run is drawn. A field the host does not define MUST be ignored rather than
rejected, since a plugin written for a later thurbox must still draw on an earlier
one.

A field that a call cannot reasonably order positionally SHALL be reachable through
the style **table** form only. The positional form is full, and one signature growing
without limit is worse than one spelling that can grow.

#### Scenario: A known token

- **WHEN** a constructor is given a defined style token
- **THEN** the resulting node carries it

#### Scenario: An unknown token

- **WHEN** a constructor is given an undefined style token
- **THEN** conversion fails naming that token

#### Scenario: A style flag that is not `true`

- **WHEN** a style names a flag as false, as a string, or as a number
- **THEN** the flag is off

#### Scenario: A field only the table form reaches

- **WHEN** a style table names the width-yielding declaration
- **THEN** the node carries it, and no positional argument could have named it

### Requirement: The plugin API is type-checkable

The host SHALL ship type definitions describing the `@thurbox` surface, and the
bundled plugin MUST pass a strict type check against them.

#### Scenario: The bundled plugin type-checks

- **WHEN** the strict Luau analyser runs over the bundled plugin
- **THEN** it reports no errors

