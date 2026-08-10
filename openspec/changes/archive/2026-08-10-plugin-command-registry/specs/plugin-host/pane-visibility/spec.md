## ADDED Requirements

### Requirement: Visibility is reachable as a command

Each declared pane's visibility SHALL be settable through the generated
`toggle`, `show`, and `hide` commands, which MUST work with no TUI running and
MUST leave the same persisted choice a user's toggle leaves.

#### Scenario: Hiding a pane headlessly

- **WHEN** a pane's hide command is invoked with no TUI running
- **THEN** the stored choice for that pane is hidden

#### Scenario: Showing a pane headlessly

- **WHEN** a pane's show command is invoked
- **THEN** the stored choice for that pane is shown

#### Scenario: Toggling a pane that has never been stored

- **WHEN** a pane's toggle command is invoked and no choice has been stored
- **THEN** the manifest's default is flipped and stored

#### Scenario: Toggling twice returns to the start

- **WHEN** a pane's toggle command is invoked twice
- **THEN** the stored choice matches where it began

#### Scenario: Each pane is independent

- **WHEN** one pane is hidden by command
- **THEN** another pane of the same plugin is unaffected

### Requirement: A running TUI honours an externally changed visibility

When another process changes a pane's stored visibility, the TUI SHALL apply it
without a restart, and MUST mark the UI dirty only when a pane's visibility
actually changed.

#### Scenario: An external hide reaches the screen

- **WHEN** a pane is visible and another process stores it as hidden
- **THEN** the TUI hides it on its next external-change poll

#### Scenario: An unchanged stored value costs no repaint

- **WHEN** the stored visibility of every pane already matches what the TUI is
  showing
- **THEN** applying it reports no change and forces no repaint

#### Scenario: A pane with no stored choice is left alone

- **WHEN** a pane has no stored choice
- **THEN** applying stored visibility leaves the pane as the manifest seeded it
