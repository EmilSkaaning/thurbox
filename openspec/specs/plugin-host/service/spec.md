# plugin-host/service Specification

## Purpose
Defines the headless half of a plugin — the part that keeps working when the
TUI is closed — so that a plugin doing background work inherits v1's guarantee
that scheduled work fires without a running UI.
## Requirements
### Requirement: A plugin may declare a service entry point

A plugin MAY ship a service entry point in addition to its view entry. A plugin
shipping only a service SHALL be valid and MUST contribute no pane; a plugin
shipping neither entry point MUST be rejected as invalid.

#### Scenario: A service-only plugin

- **WHEN** a plugin ships a service entry and no view entry
- **THEN** it loads, its service runs, and it contributes no pane

#### Scenario: A plugin with both halves

- **WHEN** a plugin ships both entries
- **THEN** both load, each in its own VM

#### Scenario: A plugin with neither

- **WHEN** a plugin ships no entry point at all
- **THEN** it fails to load, naming what is missing

### Requirement: The halves are isolated from each other

The service and view halves SHALL run in separate VMs. Neither may observe or
mutate the other's state, and a failure in one MUST NOT stop the other.

#### Scenario: State does not cross the halves

- **WHEN** the service half sets a global and the view half reads that name
- **THEN** the view half observes nothing

#### Scenario: A failing service leaves the view running

- **WHEN** a plugin's service errors on start
- **THEN** its view half still loads and its pane still renders

#### Scenario: A failing view leaves the service running

- **WHEN** a plugin's view half fails to compile
- **THEN** its service half still runs

### Requirement: A service is hosted without a TUI

A service SHALL be startable by a headless invocation, so a plugin's background
work continues when no TUI is running.

#### Scenario: Headless hosting

- **WHEN** the headless tick runs and a plugin declares a service
- **THEN** that service is started

#### Scenario: A service-only plugin needs no terminal

- **WHEN** a service-only plugin is hosted headlessly
- **THEN** it runs without any terminal, pane, or view VM being created

### Requirement: Service capabilities are granted separately

A plugin SHALL declare capabilities per half, and the host MUST grant each half
only its own set. A capability granted to the service MUST NOT appear in the
view's environment, and vice versa.

#### Scenario: A capability granted only to the service

- **WHEN** a manifest grants a capability to the service half alone
- **THEN** the binding is present in the service VM and absent from the view VM

#### Scenario: A capability granted only to the view

- **WHEN** a manifest grants a capability to the view half alone
- **THEN** the binding is absent from the service VM

