# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A fourth native pane is reproduced by a bundled plugin

A fourth of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first three: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, off screen by default, and
leaving the native pane as the one the interface draws.

The chosen pane is the **automations pane**. Its rows are composed from a schedule,
an action and a countdown, so it is the port that decides whether a composed display
string is published or its parts are; and its scroll anchor and drawn cursor come
apart, so it is the port that shows a list pane needs both.

#### Scenario: The fourth pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the automations plugin is discoverable, its manifest satisfies the same
  validation a user's plugin does, and its pane is off screen until asked for

#### Scenario: The reproduction is equal to the native tree

- **WHEN** the native pane and the plugin are given the same automations
- **THEN** the two view trees are equal, across enabled and disabled rows, each
  schedule shape, a running search with matched and filtered rows, a drawn and an
  undrawn cursor, and an empty pane in both its focus states

#### Scenario: The reproduction composes the summary thurbox composes

- **WHEN** the plugin's row summary is compared against the kernel's composition rule
  for every schedule, action and due-state combination
- **THEN** the two strings are identical

### Requirement: A port states when the reproduction cannot be placed where the native pane is

When a ported pane cannot be placed in the position its native counterpart occupies,
the port SHALL say so in its proposal, name what the host would need in order to
place it, and pin the limitation with a test that fails when the host gains the
missing capability — so the finding cannot go stale unnoticed.

The reproduction MUST still be placed somewhere it can be seen and compared, and its
equality claim MUST be stated as being about the pane's **content**, not its
placement.

#### Scenario: The pane's own column cannot be named

- **WHEN** the automations pane's plugin is written and the pane slot vocabulary
  names only the right-hand column
- **THEN** the port declares the left column out of scope, records what a left slot
  would require, and the plugin's pane is placed in the column that exists

#### Scenario: The limitation is pinned rather than described

- **WHEN** a manifest asks for the slot the native pane occupies
- **THEN** it is refused, and a test asserts that refusal so that adding the slot
  forces the finding to be revisited
