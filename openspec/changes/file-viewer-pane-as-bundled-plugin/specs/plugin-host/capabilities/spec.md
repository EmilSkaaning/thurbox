# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Reading the open file tree is its own capability

Reading the file tree thurbox has open SHALL require its own declared capability,
distinct from the capabilities that read sessions, host metrics, automations and
tasks. A plugin that declares it MUST receive the file reader and no other state
reader; a plugin that declares another state capability MUST NOT receive the file
reader.

The capability MUST NOT be named or implemented as a filesystem capability, and
declaring it MUST NOT insert any binding that performs I/O. "Reads the file tree
you have open" and "reads any file on your machine" are different sentences, and
the capability list is what an install prompt is written from.

#### Scenario: The capability grants exactly one reader

- **WHEN** a plugin declares only the file capability
- **THEN** its module table contains the file reader and none of the session,
  metrics, automation or task readers

#### Scenario: Another state capability does not imply it

- **WHEN** a plugin declares only the session capability
- **THEN** the file reader is absent from its module table

#### Scenario: It is not a filesystem grant

- **WHEN** the granted vocabulary is inspected
- **THEN** the file capability is separate from any filesystem capability, and the
  host declares no filesystem capability on its account
