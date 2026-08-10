# plugin-host/spawn-contributions Specification

## Purpose
TBD - created by archiving change plugin-spawn-contributions. Update Purpose after archive.
## Requirements
### Requirement: A plugin may add environment to spawned sessions

A plugin SHALL be able to declare environment variables that the host adds to
every agent session thurbox spawns. The declaration MUST be readable from the
manifest without executing plugin code, and the host MUST apply it on every
spawn path — headless creation, TUI creation, restart, and restore alike.

#### Scenario: A declared variable reaches the session

- **WHEN** an installed plugin declares an environment variable and a session
  is spawned
- **THEN** the spawned session's environment contains that variable with the
  declared value

#### Scenario: No plugin declares anything

- **WHEN** no installed plugin declares a spawn contribution and a session is
  spawned
- **THEN** the session's environment is exactly what the kernel alone would
  have set

#### Scenario: A build without the plugin host

- **WHEN** thurbox is built without the plugin feature and a session is spawned
- **THEN** the session's environment is exactly what the kernel alone would
  have set

### Requirement: The kernel's own environment is reserved

The host SHALL refuse a contribution that would set a variable the kernel has
already set for that spawn. The kernel's value MUST survive, and the refusal
MUST be recorded rather than discarded.

#### Scenario: A plugin targets the session identity variable

- **WHEN** a plugin declares a variable that the kernel sets to identify the
  session
- **THEN** the session's environment keeps the kernel's value
- **AND** a rejection naming the plugin and the variable is recorded

### Requirement: Code-execution environment variables are refused

The host SHALL refuse any contributed variable that would cause code to run
inside the spawned agent's processes. The refusal MUST NOT depend on the
spelling's case.

#### Scenario: A contribution names a loader variable

- **WHEN** a plugin declares a variable on the host's denied list
- **THEN** the variable is absent from the spawned session's environment
- **AND** a rejection naming the plugin and the variable is recorded

#### Scenario: The denied name is spelled in another case

- **WHEN** a plugin declares a denied variable using different letter casing
- **THEN** it is refused exactly as the canonical spelling is

### Requirement: PATH cannot be contributed

The host SHALL refuse a contribution that sets `PATH`. Replacing `PATH` would
change which program every command in the session resolves to, which is the
same power the denied variables carry.

#### Scenario: PATH set outright

- **WHEN** a plugin declares `PATH` as an environment variable
- **THEN** it is refused like any other denied variable
- **AND** a rejection naming the plugin is recorded

#### Scenario: No way to prepend

- **WHEN** a plugin's manifest attempts to declare directories to prepend to
  `PATH`
- **THEN** the manifest is rejected, because the session backend cannot deliver
  a contributed `PATH` and a declaration that did nothing would be worse than
  no declaration

### Requirement: Contributions are append-only across plugins

When two plugins contribute the same variable, the host SHALL keep the first
value in a deterministic order and MUST record the loser's attempt. The outcome
MUST NOT depend on discovery order varying between runs.

#### Scenario: Two plugins declare the same variable

- **WHEN** two installed plugins declare the same environment variable with
  different values
- **THEN** the spawned session carries exactly one of them, chosen the same way
  on every run
- **AND** a rejection naming the losing plugin, the variable and the winner is
  recorded

### Requirement: A refused contribution never fails a spawn

The host SHALL complete a spawn regardless of how many contributions were
refused. A rejection MUST NOT abort, delay, or alter any part of the spawn
other than the refused variable.

#### Scenario: Every contribution is refused

- **WHEN** a plugin's entire contribution is refused
- **THEN** the session spawns normally with the kernel's environment

### Requirement: Refusals are visible without a spawn

The host SHALL provide a report that lists, for every installed plugin, what its
declared contribution would add and what the policy refuses, derived from the
manifests alone. Producing the report MUST NOT execute plugin code.

#### Scenario: Asking for the report

- **WHEN** the operator asks for the spawn-contribution report
- **THEN** each installed plugin's accepted variables and refused variables are
  listed, each refusal carrying its reason

#### Scenario: The report starts nothing

- **WHEN** the report is produced for a plugin whose code would fail to load
- **THEN** the report is still produced and no plugin code has run

#### Scenario: A refusal at spawn time

- **WHEN** a contribution is refused while a session is being spawned
- **THEN** a warning naming the plugin, the variable and the reason is written
  to the log

