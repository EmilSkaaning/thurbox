## Purpose

Defines the headless surface that reports what the plugin host found — which
plugins loaded, which did not, and why — so that a plugin that fails to run
produces an answer a user can reach rather than silence.

## ADDED Requirements

### Requirement: A plugin listing reports every known plugin

`thurbox-cli plugin list` SHALL report every discovered plugin exactly once,
with its name, source, lifecycle state, and granted capabilities. A plugin that
failed MUST still appear.

#### Scenario: Mixed healthy and failed plugins

- **WHEN** the listing runs with one running plugin and one that failed to load
- **THEN** both appear, each with its own state

#### Scenario: No plugins installed

- **WHEN** the listing runs with no plugins discovered
- **THEN** it succeeds and reports an empty set rather than failing

### Requirement: Status explains a plugin that did not start

`thurbox-cli plugin status` SHALL report, for a failed plugin, the transition
it failed in and the cause. Given a plugin name it reports that plugin; given
none it reports all.

#### Scenario: A named plugin failed to initialize

- **WHEN** status is requested for a plugin whose `init` raised an error
- **THEN** the output names the initialize transition and carries the error
  message

#### Scenario: A named plugin does not exist

- **WHEN** status is requested for a name that was never discovered
- **THEN** the command fails with an error naming the requested plugin, rather
  than reporting an empty success

### Requirement: Doctor reports what discovery rejected

`thurbox-cli plugin doctor` SHALL report every discovery problem with its cause
and the path it concerns: invalid manifests, plugins overridden by a
higher-precedence source, same-source name conflicts, and unreadable
directories.

#### Scenario: A malformed manifest

- **WHEN** a plugin directory holds an invalid manifest
- **THEN** doctor names that path and the validation failure

#### Scenario: A shadowed plugin

- **WHEN** a user plugin overrides a bundled one of the same name
- **THEN** doctor reports the override, naming both the winner and the shadowed
  copy

#### Scenario: Nothing is wrong

- **WHEN** every discovered plugin is valid
- **THEN** doctor succeeds and reports no problems

### Requirement: Output format follows the existing CLI convention

Every `plugin` verb SHALL emit human-readable output when stdout is a terminal
and JSON when piped, and MUST honor `--json`, `--pretty`, and `--text` to force
a format — matching every other `thurbox-cli` subcommand.

#### Scenario: Piped output is machine-readable

- **WHEN** a plugin verb's stdout is not a terminal
- **THEN** it emits JSON

#### Scenario: Format is forced

- **WHEN** `--text` is passed with stdout piped
- **THEN** human-readable output is emitted anyway

### Requirement: The subcommand is absent without the feature

A build compiled without the plugin feature SHALL NOT offer the `plugin`
subcommand at all. It MUST be absent from help output rather than present and
reporting that plugins are unavailable.

#### Scenario: Stable build help

- **WHEN** help is requested from a build without the plugin feature
- **THEN** no `plugin` subcommand is listed
