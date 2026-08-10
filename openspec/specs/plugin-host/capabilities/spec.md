# plugin-host/capabilities Specification

## Purpose
Defines how a plugin declares the host powers it needs and how the host bounds
it to exactly those — the mechanism that makes a plugin's reach reviewable from
its manifest rather than discoverable only by reading its code.
## Requirements
### Requirement: Capabilities are declared in the manifest

A plugin SHALL declare every capability it requires in its manifest. The
declared set MUST be readable without executing plugin code, so that a
plugin's reach is reviewable before it runs.

#### Scenario: Manifest declares capabilities

- **WHEN** a manifest declares a set of capabilities
- **THEN** the host reports that set for the plugin without creating a VM

#### Scenario: Manifest declares no capabilities

- **WHEN** a manifest omits the capability declaration
- **THEN** the plugin is treated as requesting none

### Requirement: The capability vocabulary is closed

The host SHALL define a fixed set of recognized capability names and MUST
reject a manifest requesting a capability it does not recognize. An unknown
capability MUST NOT be silently ignored or treated as granting nothing.

#### Scenario: Manifest requests an unknown capability

- **WHEN** a manifest requests a capability the host does not define
- **THEN** the manifest is rejected with an error naming the unknown capability

#### Scenario: Manifest requests a recognized capability

- **WHEN** a manifest requests a capability the host defines
- **THEN** the manifest validates and the capability is recorded as requested

### Requirement: Enforcement is by absence, not by refusal

The host SHALL construct each plugin's environment containing only the bindings
its declared capabilities permit. A binding for an undeclared capability MUST
be absent from the environment rather than present and returning an error.

#### Scenario: An undeclared binding is absent

- **WHEN** a plugin that did not declare a capability inspects its environment
  for that capability's binding
- **THEN** the binding is absent

#### Scenario: A declared binding is present

- **WHEN** a plugin that declared a capability inspects its environment
- **THEN** that capability's binding is present and callable

#### Scenario: Two plugins with different declarations

- **WHEN** one plugin declares a capability and another does not
- **THEN** the binding is present in the first plugin's environment and absent
  from the second's

### Requirement: A plugin cannot widen its own capabilities

A plugin SHALL NOT be able to acquire a capability at runtime that its manifest
did not declare. Reaching an undeclared binding through another plugin, through
a captured reference, or by mutating its own environment MUST NOT work.

#### Scenario: A plugin attempts to reconstruct a missing binding

- **WHEN** plugin code attempts to obtain an undeclared capability's binding by
  any means available inside its VM
- **THEN** it does not obtain a working binding

#### Scenario: A plugin mutates its environment

- **WHEN** plugin code adds or replaces entries in its own environment
- **THEN** its granted capability set is unchanged

### Requirement: Capability grants are recorded per plugin

The host SHALL record which capabilities each loaded plugin was granted, and
MUST be able to report that set alongside the plugin's status.

#### Scenario: Granted set is reportable

- **WHEN** the host is asked for a running plugin's status
- **THEN** the report includes the capabilities that plugin was granted

### Requirement: Capability checks precede VM creation

The host SHALL validate a plugin's requested capabilities during manifest
validation, before its VM is created. A plugin requesting an invalid capability
MUST fail before any of its code is compiled or executed.

#### Scenario: Invalid capability rejected before load

- **WHEN** a plugin requests an unrecognized capability
- **THEN** it fails at the manifest stage
- **AND** no VM is created and no plugin source is compiled

### Requirement: Rendering is a declared capability

A plugin SHALL be asked to render only if its manifest declared the render
capability. A plugin that declares a pane without it MUST be rejected at
manifest validation, so a pane that could never draw is caught before it is
shown.

#### Scenario: A pane without the render capability

- **WHEN** a manifest declares a pane but does not request the render
  capability
- **THEN** validation fails, naming the pane and the missing capability

#### Scenario: A pane with the render capability

- **WHEN** a manifest declares a pane and requests the render capability
- **THEN** the manifest validates

#### Scenario: A plugin with the capability but no pane

- **WHEN** a manifest requests the render capability and declares no pane
- **THEN** the manifest validates, and the plugin is never asked to render

### Requirement: Receiving input is a declared capability

A plugin SHALL receive keyboard input only if its manifest declared the input
capability. The host MUST NOT deliver a key to a plugin that did not ask for
it.

#### Scenario: A plugin declares input

- **WHEN** a manifest requests the input capability
- **THEN** it validates and the plugin's panes are focusable

#### Scenario: A plugin does not declare input

- **WHEN** a manifest omits the input capability
- **THEN** its panes are not focusable and it is never handed a key

