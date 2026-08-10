## ADDED Requirements

### Requirement: Contributing to spawns is a declared capability

A plugin's spawn contribution SHALL be applied only if its manifest declared the
spawn capability. A manifest that declares a contribution without the capability
MUST be rejected at validation, so the reach is readable from the capability
list alone rather than only from the contribution's contents.

#### Scenario: A contribution without the capability

- **WHEN** a manifest declares a spawn contribution but does not request the
  spawn capability
- **THEN** validation fails, naming the missing capability

#### Scenario: A contribution with the capability

- **WHEN** a manifest declares a spawn contribution and requests the spawn
  capability
- **THEN** the manifest validates

#### Scenario: The capability without a contribution

- **WHEN** a manifest requests the spawn capability and declares no
  contribution
- **THEN** the manifest validates and nothing is contributed
