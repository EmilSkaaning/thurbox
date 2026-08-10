## ADDED Requirements

### Requirement: Capabilities are declared per half

A manifest SHALL be able to grant capabilities to the service half and the view
half independently. A capability granted to one half MUST NOT be reachable from
the other.

#### Scenario: Per-half declaration

- **WHEN** a manifest grants different capabilities to each half
- **THEN** each VM's environment carries only its own

#### Scenario: A shared declaration

- **WHEN** a manifest grants a capability without naming a half
- **THEN** both halves receive it
