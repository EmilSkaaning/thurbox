# plugin-host/pane-visibility Specification

## ADDED Requirements

### Requirement: A pane's declared action toggles it, and so does the kernel's own pane

When a pane declares a kernel action, firing that action SHALL flip that pane's
visibility through the same stored choice every other route writes. The kernel's own
pane for that seat MUST also keep doing what the action always did, so firing the
action twice returns every occupant to where it started and the kernel never loses
track of its own pane's state.

The pane MUST answer its action whether or not the kernel pane's own feature switch
is on: each occupant is gated by the switch **it** named, not by the other's. A pane
whose own switch is off MUST NOT be toggled.

Several panes declaring one action MUST each toggle, since the host cannot arbitrate
between manifests written independently.

#### Scenario: The declared action shows the pane

- **WHEN** a hidden pane declares an action and that action fires
- **THEN** the pane is shown and the choice is stored

#### Scenario: Firing it twice returns to the start

- **WHEN** the declared action fires twice
- **THEN** the pane's visibility is what it was, and so is the kernel's own pane's

#### Scenario: The kernel's pane still answers its action

- **WHEN** a pane declares the action a kernel pane already answers and the action
  fires
- **THEN** the kernel's own pane state changes as it always did

#### Scenario: A gated-off pane does not answer

- **WHEN** a pane whose declared feature switch is off is sent its declared action
- **THEN** its visibility does not change

#### Scenario: A pane with no declared action

- **WHEN** an action fires and no pane declared it
- **THEN** no pane's visibility changes

### Requirement: A pane whose feature switch is off is not a pane

A pane whose declared feature switch is off SHALL NOT be shown, MUST NOT occupy a
seat or a column, MUST NOT be focusable, MUST NOT be rendered — the host MUST NOT
enter its VM — and MUST NOT be offered by the generic plugin-pane toggle or its
picker.

Turning the switch back on MUST restore the pane to the visibility the user last
chose: the switch answers whether the pane is available, and the stored choice
answers whether the user wants it, so a switch going off and on again MUST NOT erase
a choice.

The switch MUST be read live, so a change to it applies without restarting or
reloading the plugin.

#### Scenario: A gated-off pane is not on screen

- **WHEN** a visible pane's declared feature switch is off
- **THEN** the pane is not drawn and occupies no layout space

#### Scenario: A gated-off pane is not rendered

- **WHEN** a pane's declared feature switch is off
- **THEN** the host does not enter its plugin's VM to build its tree

#### Scenario: A gated-off pane is not focusable

- **WHEN** focus moves through the panes and one pane's switch is off
- **THEN** that pane is skipped, exactly as a hidden pane is

#### Scenario: A gated-off pane is not offered by the generic toggle

- **WHEN** the generic plugin-pane toggle fires and the only other pane's switch is
  off
- **THEN** the gated-off pane is not listed and not toggled

#### Scenario: The switch comes back on

- **WHEN** a pane the user had shown is gated off and the switch is turned on again
- **THEN** the pane is shown again
