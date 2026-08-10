## MODIFIED Requirements

### Requirement: A declared pane becomes a rendered pane

A pane declared by a running plugin's manifest SHALL be available as a pane in
the UI, titled from the manifest and placed in the slot it declared. A pane
declared by a plugin that is not running MUST NOT be shown. A pane whose
visibility is off MUST NOT be shown either, even though its plugin is running.

#### Scenario: A running plugin declares a pane

- **WHEN** a plugin with a declared, visible pane reaches `running`
- **THEN** its pane is available and titled from the manifest

#### Scenario: A failed plugin declares a pane

- **WHEN** a plugin that declared a pane fails to start
- **THEN** no pane is shown for it

#### Scenario: A running plugin's pane is hidden

- **WHEN** a running plugin's pane visibility is off
- **THEN** no pane is shown for it
