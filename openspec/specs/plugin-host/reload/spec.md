# plugin-host/reload Specification

## Purpose
Defines reloading a plugin without restarting thurbox — the loop that makes
writing a pane an afternoon's work rather than a rebuild each time.
## Requirements
### Requirement: A reload rebuilds the plugin from source

Reloading a plugin SHALL stop it, discard its VM, and load and initialize it
again from its current source. No value from the previous VM may be observable
afterwards.

#### Scenario: Source changed since load

- **WHEN** a plugin's source is edited and the plugin is reloaded
- **THEN** subsequent renders reflect the new source

#### Scenario: No state survives

- **WHEN** a plugin stores a value in its VM and is then reloaded
- **THEN** the value is gone

### Requirement: A reload preserves the pane and its visibility

Reloading SHALL keep the plugin's pane in place and MUST NOT reset the user's
visibility choice.

#### Scenario: A hidden pane stays hidden across a reload

- **WHEN** a pane is hidden and its plugin is reloaded
- **THEN** the pane is still hidden

### Requirement: A failed reload leaves the plugin failed, not the host broken

If a reload fails — the new source does not compile, or `init` errors — the
plugin SHALL be recorded as failed with the cause. The host MUST keep running
and other plugins MUST be unaffected.

#### Scenario: The edited source does not compile

- **WHEN** a plugin is reloaded and its source has a syntax error
- **THEN** it is recorded as failed during load, naming the error
- **AND** other plugins keep running

#### Scenario: Recovering from a failed reload

- **WHEN** a plugin that failed to reload is reloaded again with valid source
- **THEN** it reaches running

### Requirement: A source change triggers a reload

The host SHALL detect a change to a loaded plugin's source and reload it
without user action.

#### Scenario: A file is saved

- **WHEN** a loaded plugin's entry file changes on disk
- **THEN** the plugin is reloaded

#### Scenario: Nothing changed

- **WHEN** no plugin source has changed
- **THEN** no reload happens

