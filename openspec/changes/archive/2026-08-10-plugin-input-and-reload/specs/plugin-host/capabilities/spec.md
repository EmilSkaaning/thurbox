## ADDED Requirements

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
