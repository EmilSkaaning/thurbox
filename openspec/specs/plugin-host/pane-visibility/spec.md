# plugin-host/pane-visibility Specification

## Purpose
Defines who decides whether a plugin pane is on screen — the kernel, not the
plugin — how a manifest seeds that decision, and how a user's choice survives a
restart.
## Requirements
### Requirement: Visibility is kernel-owned

The kernel SHALL own whether a pane is shown. A plugin MUST NOT be able to
force its own pane visible or hidden; a manifest only seeds the initial value.

#### Scenario: A manifest seeds the initial state

- **WHEN** a pane declares that it is visible by default and has no stored
  choice
- **THEN** it is shown on first run

#### Scenario: A pane defaults to hidden

- **WHEN** a pane declares that it is not visible by default and has no stored
  choice
- **THEN** it is not shown, even though its plugin is running

### Requirement: A user's choice persists across restarts

Once a user shows or hides a pane, the kernel SHALL persist that choice per
pane and honour it on the next launch in preference to the manifest's seed.

#### Scenario: A hidden pane stays hidden

- **WHEN** a user hides a pane that defaults to visible, and thurbox restarts
- **THEN** the pane is still hidden

#### Scenario: A shown pane stays shown

- **WHEN** a user shows a pane that defaults to hidden, and thurbox restarts
- **THEN** the pane is still shown

#### Scenario: An unknown pane falls back to its seed

- **WHEN** a pane has no stored choice
- **THEN** its manifest default decides

### Requirement: The toggle is a rebindable action

Showing and hiding the plugin pane SHALL be a rebindable action, listed in the
keybindings help alongside every other action, and MUST NOT be a hardcoded
chord.

#### Scenario: The action toggles the pane

- **WHEN** the toggle action fires and the pane is shown
- **THEN** the pane is hidden, and firing it again shows it

#### Scenario: The action is rebindable

- **WHEN** the keybindings editor lists rebindable actions
- **THEN** the plugin pane toggle appears among them

#### Scenario: No plugin pane exists

- **WHEN** the toggle fires and no plugin declares a pane
- **THEN** nothing changes and no error is raised

### Requirement: A hidden pane costs nothing to draw

A pane that is hidden SHALL NOT occupy layout space, and MUST NOT be rendered.

#### Scenario: Layout with a hidden pane

- **WHEN** every plugin pane is hidden
- **THEN** the layout is identical to one with no plugin panes at all

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

