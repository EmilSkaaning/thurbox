## ADDED Requirements

### Requirement: A reload verb

`thurbox-cli plugin reload [<name>]` SHALL reload the named plugin, or every
plugin when no name is given, reporting each outcome.

#### Scenario: Reloading a named plugin

- **WHEN** reload is run for a plugin that exists
- **THEN** the output reports its resulting state

#### Scenario: Reloading an unknown plugin

- **WHEN** reload is run for a name that was never discovered
- **THEN** the command fails naming the requested plugin

#### Scenario: Reloading everything

- **WHEN** reload is run with no name
- **THEN** every discovered plugin is reloaded and reported
