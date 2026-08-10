# migration/teardown Specification

## Purpose
TBD - created by archiving change phase-6-teardown-gate. Update Purpose after archive.
## Requirements
### Requirement: Every v1 surface scheduled for deletion is listed with what blocks it

The teardown inventory SHALL enumerate each unit v2 deletes — the v1 extension
system and each native pane — as a set of paths and in-source markers, together
with the replacement ids that MUST exist before that unit may go. Every listed
path and marker MUST still be present while any of its unit's replacements is
unready, and the check MUST name the unready ids when one is missing.

#### Scenario: A blocked unit's path is deleted

- **WHEN** a path belonging to a unit is absent from the tree and at least one of
  that unit's replacements is recorded as unready
- **THEN** the check fails, naming the missing path and every unready replacement
  id the unit requires

#### Scenario: A blocked unit's marker is removed

- **WHEN** a marker a unit lists — such as a metadata key the teardown's schema
  cleanup would drop — is no longer present in the file that carries it, while the
  unit is unready
- **THEN** the check fails, naming the marker

#### Scenario: A ready unit's path may be deleted

- **WHEN** a path belonging to a unit is absent and every replacement that unit
  requires is recorded as ready
- **THEN** the check passes for that unit

#### Scenario: Every unit names at least one requirement

- **WHEN** the inventory is read
- **THEN** no unit has an empty requirement set, so no deletion is unguarded

#### Scenario: Every requirement names a known replacement

- **WHEN** the inventory is read
- **THEN** every replacement id a unit requires resolves to a row in the
  replacement table

### Requirement: A recorded replacement verdict is verified against the build

Each capability the teardown must not lose SHALL record whether its v2 home
exists, and that verdict SHALL be re-derived from the source tree rather than
trusted. A recorded verdict that disagrees with what the tree shows MUST fail,
naming the row and the direction of the disagreement.

#### Scenario: A replacement lands and the verdict is stale

- **WHEN** a capability's v2 home now exists in the source but its recorded
  verdict is unready
- **THEN** the check fails, telling the reader to re-verdict that row

#### Scenario: A recorded verdict claims a replacement that is absent

- **WHEN** a capability is recorded as ready but its v2 home is absent from the
  source
- **THEN** the check fails, naming the row

#### Scenario: Verdicts that match the tree pass

- **WHEN** every recorded verdict equals the value its probe derives
- **THEN** the check passes

### Requirement: Teardown readiness is derived, and reported with its blockers

Whether deletion is permitted SHALL be a function of the recorded verdicts alone.
While any verdict is unready the inventory MUST report deletion as unsafe and MUST
name every blocking capability; when all verdicts are ready it MUST report
deletion as permitted.

#### Scenario: The current tree blocks deletion

- **WHEN** readiness is derived from the inventory as it stands
- **THEN** deletion is reported unsafe and the report names each unready
  capability

#### Scenario: A fully ready inventory permits deletion

- **WHEN** readiness is derived from a table in which every capability is ready
- **THEN** deletion is reported permitted with no blockers

### Requirement: The hook payloads that must survive teardown are single-sourced

Every agent wired through its own configuration directory SHALL receive the same
payload bytes locally and remotely: the asset the local installer materializes for
a manifest wiring and the payload the remote provisioner ships for that agent MUST
be identical, and each MUST be reachable from the embedded manifest.

#### Scenario: A wiring names an asset the binary does not embed

- **WHEN** a manifest wiring's source file has no entry in the embedded asset
  table
- **THEN** the check fails, naming the wiring

#### Scenario: Local and remote payloads diverge

- **WHEN** the payload the remote table ships for an agent differs from the asset
  its manifest wiring names
- **THEN** the check fails, naming the agent

#### Scenario: Every config-dir wiring agrees

- **WHEN** each config-dir wiring in the embedded manifest is compared against the
  remote table
- **THEN** destination, guard directory, delivery kind, and payload bytes all
  match

