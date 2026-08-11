# plugin-host/authoring Specification

## MODIFIED Requirements

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
