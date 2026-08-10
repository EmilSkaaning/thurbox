## ADDED Requirements

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
