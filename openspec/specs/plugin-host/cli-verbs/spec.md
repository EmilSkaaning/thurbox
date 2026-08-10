# plugin-host/cli-verbs Specification

## Purpose
Defines a plugin owning a `thurbox-cli` verb — the headless surface that lets a
plugin be scripted and cron-driven, not only clicked.
## Requirements
### Requirement: A plugin may own a CLI verb

A plugin SHALL be able to declare verbs in its manifest, and invoking
`thurbox-cli <verb>` MUST run the owning plugin with the remaining arguments.

#### Scenario: A declared verb runs

- **WHEN** a user invokes a verb an installed plugin declares
- **THEN** the plugin runs and its output is printed

#### Scenario: Arguments are passed through

- **WHEN** a verb is invoked with further arguments
- **THEN** the plugin receives them

### Requirement: A plugin verb may not shadow a kernel subcommand

A manifest declaring a verb matching a built-in `thurbox-cli` subcommand SHALL
be rejected at validation, before the plugin can load.

#### Scenario: A reserved verb

- **WHEN** a manifest declares a verb matching a kernel subcommand
- **THEN** validation fails naming that verb

#### Scenario: A kernel subcommand still resolves

- **WHEN** a built-in subcommand is invoked with plugins installed
- **THEN** the kernel handles it

### Requirement: An unknown verb is still an ordinary CLI error

A word matching no kernel subcommand and no declared plugin verb SHALL produce
the CLI's normal unknown-subcommand error, not a plugin failure.

#### Scenario: A typo

- **WHEN** a user mistypes a subcommand
- **THEN** the normal unrecognized-subcommand error is shown

### Requirement: A verb runs against the service half

A plugin verb SHALL be dispatched to the plugin's service half, so it works
with no TUI running. A plugin declaring a verb without a service half MUST fail
with an error saying so.

#### Scenario: A verb with no TUI

- **WHEN** a plugin verb is invoked and no TUI is running
- **THEN** it still runs

#### Scenario: A verb without a service half

- **WHEN** a plugin declares a verb but ships no service entry
- **THEN** invoking it fails, naming the missing service half

