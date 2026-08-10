## MODIFIED Requirements

### Requirement: A plugin verb may not shadow a kernel subcommand

A manifest declaring a verb matching a built-in `thurbox-cli` subcommand SHALL
be rejected at validation, before the plugin can load. The reserved set MUST
name every subcommand the kernel dispatches, so adding a kernel subcommand also
closes it to plugins.

#### Scenario: A reserved verb

- **WHEN** a manifest declares a verb matching a kernel subcommand
- **THEN** validation fails naming that verb

#### Scenario: A kernel subcommand still resolves

- **WHEN** a built-in subcommand is invoked with plugins installed
- **THEN** the kernel handles it

#### Scenario: The command surface is reserved

- **WHEN** a manifest declares a verb named after the kernel's command
  subcommand
- **THEN** validation fails naming it
