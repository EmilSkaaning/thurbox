# plugin-host/panes Specification

## ADDED Requirements

### Requirement: A pane that holds focus is drawn as focused

A plugin pane's frame SHALL show whether the pane holds focus, using the same
appearance thurbox's own panes use for it. A focusable pane drawn identically
whether or not it is focused would leave a user unable to see where their keys are
going.

The appearance SHALL be resolved by the kernel from the focus it owns, not published
to the plugin and not declared in the tree: a plugin is told nothing about its own
focus, and a frame is the host's.

For a pane that declared one of the kernel's pane keyboards, the level SHALL be the
level the kernel's own pane for that keyboard would have been drawn with, resolved
by one shared rule — including any intermediate level such a pane has while a
surface it opened holds focus. Two rules for one appearance is how a handed-over
pane comes to look subtly unlike the pane it replaced.

#### Scenario: A focusable pane holds focus

- **WHEN** a plugin pane that can receive keys holds focus
- **THEN** its frame is drawn as focused

#### Scenario: A focusable pane does not hold focus

- **WHEN** a plugin pane that can receive keys does not hold focus
- **THEN** its frame is drawn as unfocused

#### Scenario: A pane that cannot receive keys

- **WHEN** a plugin pane that can never receive keys is drawn
- **THEN** its frame is drawn as unfocused, exactly as before
