## Purpose

Defines who decides whether a plugin pane is on screen — the kernel, not the
plugin — how a manifest seeds that decision, and how a user's choice survives a
restart.

## ADDED Requirements

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
