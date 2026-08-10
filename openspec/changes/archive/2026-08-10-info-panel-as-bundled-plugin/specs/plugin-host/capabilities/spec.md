# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Reading kernel state is a declared capability

A plugin SHALL read kernel state only if its manifest declared the capability for
that kind of state. The capability vocabulary MUST name each kind separately, so
that what a plugin can see is readable from its manifest at the granularity a
user would care about — a plugin that renders sessions must not silently also
read host resource usage.

#### Scenario: A manifest declares a state capability

- **WHEN** a manifest requests a state-reading capability the host defines
- **THEN** the manifest validates and that capability's reader is present in the
  plugin's environment

#### Scenario: A manifest declares no state capability

- **WHEN** a manifest omits every state-reading capability
- **THEN** the plugin's environment carries no state reader at all

#### Scenario: The vocabulary stays closed

- **WHEN** a manifest requests a state-reading capability the host does not
  define
- **THEN** the manifest is rejected, naming the unknown capability
