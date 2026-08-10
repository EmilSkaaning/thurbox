# plugin-host/agent-api Specification

## Purpose
TBD - created by archiving change plugin-command-registry. Update Purpose after archive.
## Requirements
### Requirement: Commands are discoverable from the CLI

`thurbox-cli` SHALL list the registered commands and describe one in full,
without starting any plugin. Listing MAY be scoped to a single plugin.

#### Scenario: Listing every command

- **WHEN** a user lists commands with plugins installed
- **THEN** every registered command is reported with its id, plugin, title, and
  whether an agent may call it

#### Scenario: Listing with no plugins installed

- **WHEN** a user lists commands and no plugin is installed
- **THEN** the listing succeeds and reports nothing

#### Scenario: Scoping to one plugin

- **WHEN** a user lists commands for a named plugin
- **THEN** only that plugin's commands are reported

#### Scenario: Scoping to an unknown plugin

- **WHEN** a user scopes the listing to a plugin that is not installed
- **THEN** the listing fails with an error naming it, rather than reporting an
  empty list

#### Scenario: Describing one command

- **WHEN** a user describes a registered command
- **THEN** its argument schema and its caller policy are reported

#### Scenario: Describing an unknown command

- **WHEN** a user describes a command that is not registered
- **THEN** it fails with the unknown-command code

### Requirement: A command is invocable from the CLI

`thurbox-cli` SHALL invoke a registered command, accepting its arguments either
as flags or as one JSON object, and SHALL report the command's return value.

#### Scenario: Invoking with flags

- **WHEN** a user invokes a command passing each declared argument as a flag
- **THEN** the arguments bind and the command's return value is reported

#### Scenario: Invoking with a JSON object

- **WHEN** a user invokes the same command passing its arguments as one JSON
  object
- **THEN** the result is the same as with flags

#### Scenario: A bare flag for a boolean argument

- **WHEN** a boolean argument is passed as a flag with no value
- **THEN** it binds as true

#### Scenario: A bare flag for a non-boolean argument

- **WHEN** a string argument is passed as a flag with no value
- **THEN** the invocation fails with the argument-error code, saying a value is
  missing

#### Scenario: Malformed JSON arguments

- **WHEN** the JSON argument form is not a valid JSON object
- **THEN** the invocation fails with the argument-error code

#### Scenario: A global output flag typed after the command arguments

- **WHEN** a global output-format flag is passed after a command's arguments,
  where the CLI no longer matches its own flags
- **THEN** the invocation fails with the argument-error code, saying the flag
  belongs before the command

### Requirement: Failures carry a machine-readable code and a non-zero exit

An invocation that does not run the command SHALL report a structured failure
carrying a stable code and the command id, and MUST exit non-zero.

#### Scenario: An unknown command

- **WHEN** a user invokes a command id that is not registered
- **THEN** the failure carries the unknown-command code and names the id

#### Scenario: Bad arguments

- **WHEN** an invocation's arguments fail validation
- **THEN** the failure carries the argument-error code and names the offending
  argument

#### Scenario: A refused caller

- **WHEN** an agent inside a session invokes a command its policy denies
- **THEN** the failure carries the denied code

#### Scenario: An unavailable plugin

- **WHEN** a command's plugin cannot host it
- **THEN** the failure carries the unavailable code

#### Scenario: A successful invocation

- **WHEN** a command runs
- **THEN** the output carries no error code and the process exits zero

### Requirement: An agent inside a session needs no ids

An invocation SHALL resolve identity-defaulted arguments from the environment
thurbox injects into a spawned session, so an agent invoking a command passes no
session or task id. The CLI MUST NOT introduce a second identity mechanism.

#### Scenario: An agent invokes a session-scoped command

- **WHEN** an agent running inside a thurbox session invokes a command whose
  session argument defaults from the caller
- **THEN** the argument binds to the session the agent is running in, with no id
  passed

#### Scenario: The same command from a user's shell

- **WHEN** the command is invoked outside any session and the argument is
  required
- **THEN** it fails with the argument-error code naming the argument

