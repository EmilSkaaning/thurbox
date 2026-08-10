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

#### Scenario: A known token

- **WHEN** a constructor is given a defined style token
- **THEN** the resulting node carries it

#### Scenario: An unknown token

- **WHEN** a constructor is given an undefined style token
- **THEN** conversion fails naming that token

### Requirement: The plugin API is type-checkable

The host SHALL ship type definitions describing the `@thurbox` surface, and the
bundled plugin MUST pass a strict type check against them.

#### Scenario: The bundled plugin type-checks

- **WHEN** the strict Luau analyser runs over the bundled plugin
- **THEN** it reports no errors

