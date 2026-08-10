## ADDED Requirements

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
