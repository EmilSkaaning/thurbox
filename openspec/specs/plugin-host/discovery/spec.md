# plugin-host/discovery Specification

## Purpose
Defines where the host looks for plugins, in what order, and how it resolves
conflicts — so that the set of plugins present on a given machine is
predictable, and a broken plugin directory degrades to a reported error instead
of a failed startup.
## Requirements
### Requirement: Plugins are discovered from an ordered source list

The host SHALL discover plugins from an ordered list of sources: plugins
bundled into the binary first, then the user plugin directory. Discovery MUST
produce the same result for the same inputs regardless of filesystem
enumeration order.

#### Scenario: Bundled and user plugins are both present

- **WHEN** the host discovers plugins with both bundled plugins and a
  populated user plugin directory
- **THEN** the result contains plugins from both sources, each tagged with the
  source it came from

#### Scenario: Discovery is deterministic

- **WHEN** discovery runs twice over an unchanged set of sources
- **THEN** it returns the same plugins in the same order both times

### Requirement: A plugin is a directory containing a manifest

The host SHALL treat a directory containing a readable manifest file at its
root as one plugin. A directory without a manifest at its root MUST be skipped
without error, and MUST NOT be searched recursively for manifests.

#### Scenario: Directory contains a manifest

- **WHEN** the user plugin directory contains a subdirectory with a manifest at
  its root
- **THEN** that subdirectory is discovered as one plugin

#### Scenario: Directory contains no manifest

- **WHEN** the user plugin directory contains a subdirectory with no manifest
  at its root
- **THEN** the subdirectory is skipped, no error is reported, and its
  descendants are not searched

#### Scenario: A loose file in the plugin directory

- **WHEN** the user plugin directory contains a regular file rather than a
  directory
- **THEN** it is skipped without error

### Requirement: The user plugin directory is optional

A missing user plugin directory SHALL NOT be an error, and the host MUST NOT
create it as a side effect of discovery.

#### Scenario: User plugin directory does not exist

- **WHEN** discovery runs and the user plugin directory is absent
- **THEN** discovery succeeds with only bundled plugins
- **AND** the directory is not created

#### Scenario: User plugin directory cannot be read

- **WHEN** the user plugin directory exists but cannot be read
- **THEN** discovery succeeds with only bundled plugins
- **AND** the failure is recorded as a discovery error naming the path

### Requirement: A later source overrides an earlier one for the same id

When two discovered plugins declare the same name, the plugin from the later
source in the discovery order SHALL win, and the shadowed plugin MUST be
recorded as overridden. This is how a user replaces a bundled plugin with their
own copy.

#### Scenario: A user plugin shadows a bundled plugin

- **WHEN** a user plugin and a bundled plugin declare the same name
- **THEN** the user plugin is the one loaded
- **AND** the bundled plugin is recorded as overridden, naming both paths

#### Scenario: Two plugins in the same source share a name

- **WHEN** two directories in the user plugin directory declare the same name
- **THEN** both are rejected and the conflict is reported naming the shared
  name and both paths
- **AND** neither is loaded, because neither can be preferred deterministically

### Requirement: A malformed plugin never blocks discovery

A plugin whose manifest is invalid SHALL be recorded as a discovery failure and
skipped. Discovery MUST continue over the remaining plugins and MUST NOT abort
startup.

#### Scenario: One plugin among several is malformed

- **WHEN** one discovered plugin has an invalid manifest and the others are
  well-formed
- **THEN** the well-formed plugins are discovered normally
- **AND** the malformed one is reported as a failure naming its path and the
  validation error

#### Scenario: Every plugin is malformed

- **WHEN** every discovered plugin has an invalid manifest
- **THEN** discovery succeeds with no loadable plugins and a failure recorded
  for each
- **AND** startup proceeds

### Requirement: Discovery results are inspectable

The host SHALL expose the outcome of discovery — the loadable plugins, the
overridden ones, and the failures with their causes — so that a user can tell
why a plugin they installed is not running.

#### Scenario: A user asks why a plugin is missing

- **WHEN** a plugin was skipped, overridden, or failed validation
- **THEN** the discovery outcome names that plugin, its path, and the specific
  reason it is not loadable

