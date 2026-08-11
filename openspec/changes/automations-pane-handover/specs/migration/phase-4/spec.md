# migration/phase-4 Specification

## MODIFIED Requirements

### Requirement: A sixth native pane is reproduced by a bundled plugin

A sixth of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first five: shipped inside the binary, written against declared
capabilities only, and producing the native pane's view tree.

The chosen pane is the **automations pane**, the last native pane with no plugin at
all. Its rows are composed from a schedule, an action and a countdown, so it is the
port that decides whether a composed display string is published or its parts are;
its scroll anchor and drawn cursor come apart, so it is the port that shows a list
pane needs both; and it is the first pane whose keys are portable, so it is the port
that makes the plugin's own cursor real.

The port's "off screen by default, leaving the native pane as the one the interface
draws" clauses bound it **while it was a reproduction**. That pane has since been
handed over: the native renderer is deleted, the plugin is the pane, and it seeds
visible because the native band always was. The reproduction clauses still bind the
next port; they MUST NOT be read as forbidding the handover they were the
precondition for.

#### Scenario: The sixth pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the automations plugin is discoverable and its manifest satisfies the same
  validation a user's plugin does

#### Scenario: The reproduction is equal to the native tree

- **WHEN** the pane's recorded expectations — generated from the native builder while
  it existed — are compared against the plugin's trees
- **THEN** the two are equal, across enabled and disabled rows, each schedule shape, a
  running search with matched and filtered rows, a drawn and an undrawn cursor, and an
  empty pane in both its focus states

#### Scenario: The reproduction composes the summary thurbox composes

- **WHEN** the plugin's row summary is compared against the kernel's composition rule
  for every schedule, action and due-state combination
- **THEN** the two strings are identical

### Requirement: A ported pane's keys act through the plugin's own cursor

Where a pane's keys are ported **to the plugin**, the plugin SHALL move its own cursor
and act on the row that cursor names, addressed by the id the published row carries. It
MUST NOT act on the row the *kernel's* published cursor names, because that row is
wherever the native pane's cursor was left and is not what a user driving the plugin's
pane is looking at.

The port MUST declare both the input capability and the write capability its keys
need, and MUST NOT declare a key it cannot deliver an effect for.

Each ported key MUST have the effect the native key has, verified through the same
storage the native key writes rather than by inspecting the plugin's own state.

A key that changes a record MUST be **refused** while no cursor is drawn in the
pane, rather than acting on the last row the kernel's cursor happened to name. A
plugin is not told whether its own pane holds focus, so before a movement key has
given the pane a cursor of its own there is no row the user can see, and acting
would change a record nobody selected.

This requirement governs the **plugin-keys** route only. A pane handed over on the
kernel-keyboard route holds no keys at all, and its handover MUST remove the bindings
and the write capability rather than keeping them unused. Where the pane that
demonstrated this requirement is handed over that way, the demonstration MUST be
preserved in the decision record: the rules still bind any pane that wants keys of its
own, and deleting them with the tests would lose the only statement of what such a pane
must do.

#### Scenario: The cursor a key moves is the plugin's

- **WHEN** a pane holding its own keys is focused and its movement key is pressed
- **THEN** the plugin's list names a different row and the published section's cursor
  is unchanged

#### Scenario: A key's effect is the native key's effect

- **WHEN** such a pane's toggle, run and delete keys act on the row its cursor names
- **THEN** the record's flags, its due time and its deletion are what the kernel's own
  keys produce, observed in the database

#### Scenario: A write with no visible cursor is refused

- **WHEN** such a pane is focused and a record-changing key arrives before any
  movement key, so the pane draws no cursor
- **THEN** the key is reported unconsumed and no record changes

#### Scenario: Running remains a request the kernel fulfils

- **WHEN** such a pane's run key acts on an automation whose action runs a shell
  command
- **THEN** the automation is marked due and nothing is executed on a plugin thread

#### Scenario: A handover onto the kernel keyboard drops the grants

- **WHEN** the pane that demonstrated this requirement is handed over on the
  kernel-keyboard route
- **THEN** its bindings, its input capability and its write capability are removed, and
  the demonstration is preserved in the decision record

### Requirement: A key surface a plugin cannot complete is recorded with the power it needs

A ported pane's keys that cannot be reproduced SHALL be recorded individually with
the host power each would need, rather than the pane's key surface being described as
portable or unportable as a whole.

For the automations pane two of seven keys were not ported and the record MUST name
why: creating an automation needs a creation binding the mutating surface is defined
not to have, and opening the central-pane editor needs a seat the pane-slot
vocabulary does not offer, a focus a plugin cannot take, and the text authoring the
automation-write capability is defined to exclude.

Where the pane is later handed over on the kernel-keyboard route, those two keys are
answered by the **kernel** and not by any grant. The record MUST then state that the
powers it named were never granted, so that "the widening was unnecessary" stays
distinguishable from "the widening happened" — and MUST NOT be deleted as obsolete,
since it is the reason the handover needed no capability.

#### Scenario: The unported keys are itemised

- **WHEN** the port's record is read
- **THEN** each unported key is named with the host power it would need, and the
  ported keys are not described as the whole key surface

#### Scenario: The powers named were never granted

- **WHEN** the pane is handed over without them
- **THEN** the record states that each named power is still absent from the host

### Requirement: A wrap between two panes stays kernel-owned

Where two native panes form one continuous list — a movement key at the edge of one
moving focus into the other — that wrap SHALL remain the kernel's when one of the
panes becomes a plugin. Moving focus is view state, and no capability writes it.

While the pane holds its **own** keys, the plugin's share of the wrap is to
**decline** the key at its edge, which is what a consumed/not-consumed answer is for.
The port MUST record that the kernel's share — resolving an unconsumed movement key into
a focus change — is not implemented, so the key visibly does nothing at that edge, and
MUST NOT substitute a behaviour the native pane does not have (such as wrapping the
plugin's own cursor) and present it as parity.

The port MUST record that a wrap is a claim about adjacency, so it becomes
expressible only when the plugin's pane can sit where the native pane sits.

A **handover** onto the kernel keyboard closes the wrap without implementing that
kernel share at all, and the handover MUST record why: the handed-over pane is focused as
the kernel's own pane of that name, so both ends of the wrap are kernel focuses whoever
draws either pane, and the existing handlers complete it unchanged. The wrap therefore
needs no owner assigned, and survives one handover, both, or neither.

The handover MUST change the wrap's **condition** from the target pane's feature flag to
"a pane provides that list". The flag was a proxy that held only while the kernel drew
the target pane unconditionally; kept, a movement key at the edge would move focus into a
pane that is not on screen.

The reproduction's own declining half MUST be removed by that handover, since on the
keyboard route the plugin is never asked.

#### Scenario: The plugin declines at its edge

- **WHEN** a pane holding its own keys has its cursor on its first row and the
  previous-row key arrives
- **THEN** the plugin reports the key unconsumed

#### Scenario: Nothing completes the wrap

- **WHEN** an unconsumed key falls through from a focused plugin pane holding its own
  keys
- **THEN** no kernel action resolves it into a focus change

#### Scenario: The handover completes the wrap through the kernel's focus

- **WHEN** the pane is handed over on the kernel-keyboard route and a movement key is
  pressed at the adjacent pane's edge
- **THEN** focus moves into the handed-over pane exactly as before, and the plugin's
  declining half is gone

#### Scenario: The wrap's condition is the pane, not the flag

- **WHEN** no pane provides the target list
- **THEN** the movement key does not move focus into it
