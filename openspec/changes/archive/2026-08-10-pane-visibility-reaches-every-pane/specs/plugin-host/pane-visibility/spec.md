# plugin-host/pane-visibility Specification

## MODIFIED Requirements

### Requirement: The toggle is a rebindable action

Showing and hiding plugin panes SHALL be one rebindable action, listed in the
keybindings help alongside every other action, and MUST NOT be a hardcoded
chord. The action MUST reach **every** declared pane, not only the first:

- with no declared pane it does nothing and raises no error;
- with exactly one declared pane it toggles that pane;
- with two or more declared panes it opens a kernel-owned picker listing every
  declared pane, plugin-qualified, each showing whether it is on screen.

Whichever route is taken, the resulting visibility MUST be persisted through the
same per-pane stored choice a generated visibility command writes, so a toggle
from the keyboard and a `hide` command are indistinguishable afterwards.

#### Scenario: The action toggles the pane

- **WHEN** one pane is declared, it is shown, and the toggle action fires
- **THEN** the pane is hidden, firing it again shows it, and no picker opens

#### Scenario: The action opens a picker for two panes

- **WHEN** two panes are declared and the toggle action fires
- **THEN** a picker opens listing both panes with their current visibility, and
  neither pane's visibility has changed yet

#### Scenario: A pane other than the first is reachable

- **WHEN** the picker is open and the user selects the second pane and toggles it
- **THEN** that pane's visibility flips, the first pane's is untouched, and the
  new value is stored for that pane

#### Scenario: The picker's toggle stores the same choice a command does

- **WHEN** a pane is toggled from the picker
- **THEN** the stored choice for that pane equals what the pane's generated
  toggle command would have stored

#### Scenario: Leaving the picker changes nothing further

- **WHEN** the picker is dismissed
- **THEN** every pane keeps the visibility it had when the picker closed

#### Scenario: The action is rebindable

- **WHEN** the keybindings editor lists rebindable actions
- **THEN** the plugin pane toggle appears among them, as one action however many
  panes are declared

#### Scenario: No plugin pane exists

- **WHEN** the toggle fires and no plugin declares a pane
- **THEN** nothing changes, no picker opens, and no error is raised

### Requirement: A hidden pane costs nothing to draw

A pane that is hidden SHALL NOT occupy layout space, MUST NOT be painted, and
MUST NOT be rendered — the host MUST NOT enter a plugin's VM to build a tree for
a pane the kernel is keeping off screen.

A pane the host has not been told about MUST be treated as visible, so a host
running without a publisher renders exactly as it did before this rule existed.

#### Scenario: Layout with a hidden pane

- **WHEN** every plugin pane is hidden
- **THEN** the layout is identical to one with no plugin panes at all

#### Scenario: A hidden pane is not rendered

- **WHEN** one of two declared panes is published as hidden and the host renders
  its panes
- **THEN** exactly one VM render happens, and only the visible pane's result is
  returned

#### Scenario: A pane the host was never told about

- **WHEN** nothing has been published about a pane's visibility and the host
  renders its panes
- **THEN** that pane is rendered

#### Scenario: Unhiding restores the render

- **WHEN** a pane published as hidden is published as visible and the host
  renders again
- **THEN** that pane is rendered

## ADDED Requirements

### Requirement: What the host is told about visibility is bounded work

The kernel SHALL publish which panes are hidden for the host to read, and MUST
write that publication only when the set of hidden panes actually changed — a
tick on which no pane's visibility moved MUST cost no publication.

The publication MUST be observable as a counter, so a regression that makes it
per-tick work is a failing test rather than a profile someone has to run.

#### Scenario: A change is published

- **WHEN** a pane is hidden
- **THEN** the published hidden set names that pane and the publication counter
  advances

#### Scenario: An unchanged set is not republished

- **WHEN** the tick runs repeatedly with no pane's visibility changing
- **THEN** the publication counter does not advance

#### Scenario: A build with no plugin panes publishes nothing

- **WHEN** no plugin declares a pane
- **THEN** no publication happens and the counter stays at zero
