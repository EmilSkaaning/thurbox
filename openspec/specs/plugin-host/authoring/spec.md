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

### Requirement: A plugin can trim a string by the kernel's own whitespace rule

The host module SHALL provide a `trim` function that removes leading and trailing
whitespace from a string by the **same** rule the kernel trims by — Unicode's
`White_Space` property, not an ASCII byte class — and returns what is left.

It exists because a plugin cannot write it. The VM's pattern classes are byte
predicates, so a no-break space, a figure space or an ideographic space is invisible to
them; a pane reproducing a kernel row that trims its text is therefore off by a column
for every whitespace character outside ASCII, and no pattern closes it. Enumerating code
points in the plugin would be a second definition of whitespace that is wrong whenever
the first one grows.

It MUST be present for every plugin regardless of the capabilities it declared, and MUST
NOT be guarded by one: it is a pure function of its argument, reads no kernel state,
reaches nothing outside the VM and cannot fail. Adding it MUST NOT add a capability, and
MUST NOT add a binding under an existing one.

The host MUST NOT close this gap by trimming on a plugin's behalf — neither by
publishing text already trimmed, nor by trimming inside a node constructor. Which text a
pane shows, and whether whitespace-only counts as nothing, are the pane's decisions; only
the *predicate* is the kernel's. A constructor that trimmed its own content would also
make a deliberate leading space undrawable, which panes rely on for glyph padding and
indentation.

#### Scenario: Whitespace outside ASCII is trimmed

- **WHEN** a plugin trims a string padded with a no-break space
- **THEN** the padding is removed, exactly as the kernel's own trim removes it

#### Scenario: The answer is the kernel's answer

- **WHEN** a plugin trims any string
- **THEN** the result is what the kernel's trim returns for that string

#### Scenario: A plugin with no capabilities can trim

- **WHEN** a plugin that declared no capabilities calls the function
- **THEN** it is present and returns the trimmed string

#### Scenario: Trimming grants nothing

- **WHEN** the granted capability surface is inspected before and after the function is
  added
- **THEN** the capability vocabulary is unchanged and no capability's bindings have grown

#### Scenario: Trimming on the plugin's behalf is refused

- **WHEN** it is proposed that the kernel publish the trimmed text instead, or that a
  text node trim its own content
- **THEN** both are refused — the first moves a presentation decision into the
  publication, the second makes a deliberate leading space undrawable

