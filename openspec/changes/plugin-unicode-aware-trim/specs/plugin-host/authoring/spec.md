# plugin-host/authoring delta

## ADDED Requirements

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
