# plugin-host/lifecycle Specification

## Purpose
Defines the states a plugin moves through from discovery to shutdown, what the
host guarantees at each transition, and what happens when a plugin fails —
so that a plugin's status is always a definite, inspectable answer rather than
an inference from missing output.
## Requirements
### Requirement: A plugin occupies exactly one lifecycle state

Every known plugin SHALL be in exactly one of: `discovered`, `loaded`,
`running`, `stopped`, or `failed`. The host MUST be able to report the current
state of every known plugin, and a `failed` plugin MUST carry the cause and the
state it failed in.

#### Scenario: State is reported for every known plugin

- **WHEN** the host is asked for plugin status
- **THEN** every discovered plugin appears exactly once with one state

#### Scenario: A failed plugin carries its cause

- **WHEN** a plugin has failed
- **THEN** its status carries the failing transition and the error message

### Requirement: Transitions follow a defined order

A plugin SHALL progress `discovered` → `loaded` → `running`, and MUST reach
`stopped` only from `running`. Any transition MAY instead reach `failed`. The
host MUST NOT skip a transition, and MUST NOT initialize a plugin whose source
has not compiled.

#### Scenario: Normal progression

- **WHEN** a valid plugin is loaded and initialized
- **THEN** it passes through `discovered`, `loaded`, and `running` in that
  order

#### Scenario: Compilation failure stops progression

- **WHEN** a plugin's source fails to compile
- **THEN** it becomes `failed` in the `loaded` transition
- **AND** its initialization entry point is never called

### Requirement: Initialization receives a plugin-scoped context

The host SHALL call each plugin's initialization entry point exactly once per
load, passing a context scoped to that plugin. The context MUST expose the
plugin's own identity and only the host bindings its manifest's capabilities
permit.

#### Scenario: Initialization is called once

- **WHEN** a plugin is loaded successfully
- **THEN** its initialization entry point is called exactly once

#### Scenario: Context reflects declared capabilities

- **WHEN** a plugin's initialization context is inspected
- **THEN** it exposes bindings only for capabilities the plugin's manifest
  requested

#### Scenario: Plugin has no initialization entry point

- **WHEN** a plugin's module returns no initialization entry point
- **THEN** the plugin reaches `running` without one being called

### Requirement: A failing plugin does not affect other plugins

A plugin that fails at any transition SHALL be recorded as `failed` and
skipped. The host MUST continue loading and initializing the remaining plugins,
and MUST NOT abort startup.

#### Scenario: One plugin fails to initialize

- **WHEN** one plugin errors during initialization and others do not
- **THEN** the failing plugin is `failed` and the others reach `running`

#### Scenario: Every plugin fails

- **WHEN** every plugin fails to load
- **THEN** the host starts normally with no running plugins

### Requirement: Initialization order is deterministic and independent

The host SHALL initialize plugins in a deterministic order derived from
discovery. A plugin MUST NOT be able to depend on another plugin having been
initialized first; there are no declared inter-plugin dependencies.

#### Scenario: Order is stable across runs

- **WHEN** the same set of plugins is initialized twice
- **THEN** the initialization order is identical both times

#### Scenario: A plugin observes no other plugin at init

- **WHEN** a plugin initializes
- **THEN** it has no means of reading another plugin's state

### Requirement: Shutdown stops every running plugin

On shutdown the host SHALL stop every `running` plugin and release its VM and
thread. A plugin that errors or hangs while stopping MUST NOT prevent the host
from exiting, and MUST NOT prevent other plugins from being stopped.

#### Scenario: Clean shutdown

- **WHEN** the host shuts down with running plugins
- **THEN** each is stopped and its VM and thread are released

#### Scenario: A plugin hangs during shutdown

- **WHEN** a plugin does not stop within the shutdown budget
- **THEN** it is abandoned and recorded as failed to stop
- **AND** the host still exits and the remaining plugins are still stopped

### Requirement: The lifecycle admits reloading without redesign

The state machine SHALL be defined so that returning a `running` plugin to
`discovered` and re-running the load and initialize transitions is a valid
sequence, even though nothing triggers it in this change.

#### Scenario: A stopped plugin is loaded again

- **WHEN** a plugin is stopped and then loaded and initialized again
- **THEN** it reaches `running` with a fresh VM
- **AND** no state from its previous VM is observable

### Requirement: The host starts during boot and stops during shutdown

Both binaries SHALL start the plugin host as part of their startup and stop it
as part of their shutdown. The TUI and the headless CLI MUST discover the same
plugin set from the same sources, so the two never disagree about what is
installed.

#### Scenario: The TUI boots with plugins installed

- **WHEN** the TUI starts and a valid plugin is present
- **THEN** that plugin reaches `running` without user action

#### Scenario: Shutdown stops the host

- **WHEN** the TUI exits
- **THEN** every running plugin is stopped and its VM and thread released

#### Scenario: Both binaries agree

- **WHEN** the TUI and the headless CLI run against the same plugin directory
- **THEN** they report the same plugins with the same states

### Requirement: Plugin startup does not delay the first frame

Starting plugins SHALL NOT block the TUI's first frame. A plugin that is slow
or hangs during load or `init` MUST NOT prevent the UI from drawing or
accepting input, and MUST NOT delay any other plugin from starting.

#### Scenario: A plugin hangs during init

- **WHEN** a plugin's `init` does not return
- **THEN** the TUI still draws its first frame and accepts input
- **AND** the hanging plugin is recorded as failed once its execution bound
  trips

#### Scenario: A slow plugin does not hold up another

- **WHEN** one plugin takes far longer to start than another
- **THEN** the faster plugin reaches `running` without waiting for the slower

### Requirement: Startup failures are logged

A plugin that fails to start SHALL be logged with its name, the transition it
failed in, and the cause, so the failure is discoverable without running a
command.

#### Scenario: A plugin fails to compile at boot

- **WHEN** a plugin's source does not compile during startup
- **THEN** a log entry names the plugin, the load transition, and the compile
  error

#### Scenario: A healthy startup is quiet

- **WHEN** every plugin starts successfully
- **THEN** no failure is logged

