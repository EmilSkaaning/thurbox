## ADDED Requirements

### Requirement: A command declaration carries its documentation and policy

A `[[commands]]` entry MAY declare a description, whether an agent may call it,
and its caller policy. Omitting them MUST yield conservative-by-documentation
defaults: agent-callable with an allowing policy, since a command that no
surface can reach is useless and every generated command is reversible.

#### Scenario: A fully specified command

- **WHEN** a manifest declares a command with a title, description, agent
  callability, and a policy
- **THEN** the manifest validates and each value is readable from it

#### Scenario: A minimal command

- **WHEN** a manifest declares a command with only an id
- **THEN** it validates, is agent-callable, and its policy allows

#### Scenario: An unrecognized policy

- **WHEN** a manifest declares a caller policy the host does not implement
- **THEN** the manifest is rejected with an error naming the policies that exist

#### Scenario: A misspelled key in a command

- **WHEN** a command declaration carries a key the host does not define
- **THEN** the manifest is rejected with an error naming the key

### Requirement: A command declares typed arguments

A command MAY declare arguments, each with an identifier name and one of the
types `string`, `integer`, or `boolean`. A repeated argument name, a malformed
name, or an unrecognized type MUST be a manifest error.

#### Scenario: Typed arguments validate

- **WHEN** a command declares a required string argument and an optional integer
  argument
- **THEN** the manifest validates and both are readable with their types

#### Scenario: A repeated argument name

- **WHEN** a command declares the same argument name twice
- **THEN** the manifest is rejected naming the argument

#### Scenario: A malformed argument name

- **WHEN** an argument name breaks the identifier rules
- **THEN** the manifest is rejected naming the argument and the rule

#### Scenario: An unrecognized argument type

- **WHEN** an argument declares a type the host does not define
- **THEN** the manifest is rejected naming the type

### Requirement: An argument may not be named after a global CLI flag

An argument name matching one of `thurbox-cli`'s global output-format flags
SHALL be a manifest error. Such an argument could never receive its value,
because the CLI matches its own global flags before a command's arguments.

#### Scenario: An argument named after an output flag

- **WHEN** a command declares an argument named after a global output-format
  flag
- **THEN** the manifest is rejected with an error naming that flag

#### Scenario: An ordinary argument name

- **WHEN** a command declares an argument whose name matches no global flag
- **THEN** the manifest validates

### Requirement: An identity default is restricted to string arguments

An argument MAY declare that its value defaults from the calling session or the
calling task. Declaring it on an argument that is not a `string` MUST be a
manifest error, because the identity it fills is an id.

#### Scenario: An identity default on a string

- **WHEN** a string argument declares that it defaults from the calling session
- **THEN** the manifest validates

#### Scenario: An identity default on an integer

- **WHEN** an integer argument declares an identity default
- **THEN** the manifest is rejected naming the argument

#### Scenario: An unrecognized identity source

- **WHEN** an argument declares an identity source the host does not define
- **THEN** the manifest is rejected naming the sources that exist

