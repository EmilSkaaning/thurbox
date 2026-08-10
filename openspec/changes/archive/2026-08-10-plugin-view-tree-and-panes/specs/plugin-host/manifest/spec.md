## ADDED Requirements

### Requirement: A pane declares the slot it occupies

A `[[panes]]` entry SHALL declare which slot its pane occupies, drawn from a
closed set the host defines. A pane declaring an unrecognized slot MUST be
rejected at manifest validation, before any VM is created.

#### Scenario: A pane declares a known slot

- **WHEN** a manifest declares a pane with a slot the host defines
- **THEN** the manifest validates and the pane carries that slot

#### Scenario: A pane declares an unknown slot

- **WHEN** a manifest declares a pane with an unrecognized slot
- **THEN** validation fails naming the offending slot

#### Scenario: A pane omits its slot

- **WHEN** a manifest declares a pane with no slot
- **THEN** the pane takes the host's default slot
