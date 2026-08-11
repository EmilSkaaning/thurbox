# plugin-host/pane-visibility Specification

## MODIFIED Requirements

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

**A pane drawn in a deleted native pane's place SHALL seed at the visibility that
native pane defaulted to.** The exemption above permits such a pane to seed visible;
it does not ask it to. A handed-over pane that seeds visible when the pane it
replaces defaulted to hidden puts a column on every wide install that nobody asked
for — the same harm as an un-exempted reproduction, arrived at by a different route,
and with the duplication that made the reproduction obviously wrong now absent. So
the exemption MUST be read as removing the duplication objection only, and the seed
MUST still be argued from what the native pane did.

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

#### Scenario: A handed-over pane inherits the native pane's default

- **WHEN** a bundled pane is drawn in the place of a native pane that defaulted to
  hidden
- **THEN** it seeds hidden too, so the first launch after the handover looks like
  the last launch before it and the pane's toggle is what puts it on screen

### Requirement: A pane's declared action toggles it, and so does the kernel's own pane

When a pane declares a kernel action, firing that action SHALL flip that pane's
visibility through the same stored choice every other route writes. The kernel's own
pane for that seat MUST also keep doing what the action always did, **for as long as
the kernel has a pane for that seat**, so firing the action twice returns every
occupant to where it started and the kernel never loses track of its own pane's
state.

A **handover** deletes the kernel's occupant of one seat. For the action bound to
that seat there is then exactly one occupant, and firing the action twice MUST still
return it to where it started. The kernel MUST NOT retain a visibility flag for a
pane it no longer draws: such a flag can still carve the seat, so an action that
flipped it would produce a bordered column nothing paints.

When no pane at all answers the action — the plugin failed to load, a user's own
plugin of the same name declares no such pane, or the build has no plugin host — the
action SHALL report what provides the pane. Silence is indistinguishable from a
broken keybinding.

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
  where the kernel still has one

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

#### Scenario: An action whose kernel pane has been handed over

- **WHEN** the action bound to a handed-over seat fires twice
- **THEN** the plugin pane is the only occupant that flips, and it ends where it
  started, with no kernel flag left to carve the seat on its own

#### Scenario: Nothing answers a handed-over pane's action

- **WHEN** the action bound to a handed-over seat fires and no pane claims that seat
- **THEN** nothing is drawn there and the action reports which plugin provides the
  pane, rather than consuming the key silently
