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

### Requirement: No bundled pane is on screen before a user asks for it

While a bundled plugin's pane reproduces a native pane rather than replacing it,
or exists as a worked example rather than as a pane a user asked for, its manifest
SHALL seed it hidden. The rule binds the whole bundled set, not the panes that
happen to have remembered it: a build that ships the host MUST look like the build
before it until the user shows something.

The seed defaults to visible, which is right for a plugin an author installed on
purpose and wrong for one that arrives inside the binary. So the bundled set MUST
be checked rather than reviewed — a bundled manifest that declares a visible pane
MUST fail unless that pane is the one drawn in a native pane's place.

#### Scenario: A reproduction pane ships hidden

- **WHEN** a bundled plugin reproduces a native pane that the application still
  draws
- **THEN** its manifest seeds the pane hidden, so a fresh launch shows one of that
  pane rather than two

#### Scenario: An example pane ships hidden

- **WHEN** a bundled plugin exists as a worked example of the plugin contract
- **THEN** its manifest seeds its pane hidden, so a fresh launch shows no pane the
  user did not ask for

#### Scenario: A bundled manifest omits the seed

- **WHEN** a bundled manifest declares a pane without saying whether it is visible
- **THEN** the check fails, because the seed's default is visible and would put
  that pane on every install

