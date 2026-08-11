# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A sixth native pane is reproduced by a bundled plugin

A sixth of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first five: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, off screen by default, and
leaving the native pane as the one the interface draws.

The chosen pane is the **automations pane**, the last native pane with no plugin at
all. Its rows are composed from a schedule, an action and a countdown, so it is the
port that decides whether a composed display string is published or its parts are;
its scroll anchor and drawn cursor come apart, so it is the port that shows a list
pane needs both; and it is the first pane whose keys are portable, so it is the port
that makes the plugin's own cursor real.

#### Scenario: The sixth pane's plugin ships and loads

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

### Requirement: A ported pane's keys act through the plugin's own cursor

Where a pane's keys are ported, the plugin SHALL move its own cursor and act on the
row that cursor names, addressed by the id the published row carries. It MUST NOT act
on the row the *kernel's* published cursor names, because that row is wherever the
native pane's cursor was left and is not what a user driving the plugin's pane is
looking at.

The port MUST declare both the input capability and the write capability its keys
need, and MUST NOT declare a key it cannot deliver an effect for.

Each ported key MUST have the effect the native key has, verified through the same
storage the native key writes rather than by inspecting the plugin's own state.

A key that changes a record MUST be **refused** while no cursor is drawn in the
pane, rather than acting on the last row the kernel's cursor happened to name. A
plugin is not told whether its own pane holds focus, so before a movement key has
given the pane a cursor of its own there is no row the user can see, and acting
would change a record nobody selected.

#### Scenario: The cursor a key moves is the plugin's

- **WHEN** the plugin pane is focused and its movement key is pressed
- **THEN** the plugin's list names a different row and the published section's cursor
  is unchanged

#### Scenario: A key's effect is the native key's effect

- **WHEN** the plugin's toggle, run and delete keys act on the row its cursor names
- **THEN** the automation's enabled flag, its due time and its deletion are what the
  native pane's keys produce, observed in the database

#### Scenario: A write with no visible cursor is refused

- **WHEN** the plugin pane is focused and a record-changing key arrives before any
  movement key, so the pane draws no cursor
- **THEN** the key is reported unconsumed and no record changes

#### Scenario: Running remains a request the kernel fulfils

- **WHEN** the plugin's run key acts on an automation whose action runs a shell
  command
- **THEN** the automation is marked due and nothing is executed on a plugin thread

### Requirement: A key surface a plugin cannot complete is recorded with the power it needs

A ported pane's keys that cannot be reproduced SHALL be recorded individually with
the host power each would need, rather than the pane's key surface being described as
portable or unportable as a whole.

For the automations pane two of seven keys are not ported and the record MUST name
why: creating an automation needs a creation binding the mutating surface is defined
not to have, and opening the central-pane editor needs a seat the pane-slot
vocabulary does not offer, a focus a plugin cannot take, and the text authoring the
automation-write capability is defined to exclude.

#### Scenario: The unported keys are itemised

- **WHEN** the port's record is read
- **THEN** each unported key is named with the host power it would need, and the
  ported keys are not described as the whole key surface

### Requirement: A wrap between two panes stays kernel-owned

Where two native panes form one continuous list — a movement key at the edge of one
moving focus into the other — that wrap SHALL remain the kernel's when one of the
panes becomes a plugin. Moving focus is view state, and no capability writes it.

The plugin's share of the wrap is to **decline** the key at its edge, which is what a
consumed/not-consumed answer is for. The port MUST record that the kernel's share —
resolving an unconsumed movement key into a focus change — is not implemented, so the
key visibly does nothing at that edge, and MUST NOT substitute a behaviour the native
pane does not have (such as wrapping the plugin's own cursor) and present it as parity.

The port MUST record that a wrap is a claim about adjacency, so it becomes
expressible only when the plugin's pane can sit where the native pane sits.

#### Scenario: The plugin declines at its edge

- **WHEN** the plugin pane's cursor is on its first row and the previous-row key
  arrives
- **THEN** the plugin reports the key unconsumed

#### Scenario: Nothing completes the wrap

- **WHEN** an unconsumed key falls through from a focused plugin pane
- **THEN** no kernel action resolves it into a focus change, and a test asserts that,
  so adding one forces the finding to be revisited

## MODIFIED Requirements

### Requirement: A pane's key surface is ported only when a plugin pane can own the cursor those keys move

A native pane's keys SHALL be reproduced by a plugin only when the plugin can act on
the row the user is looking at. Where it cannot, the port MUST leave the pane's keys
with the kernel and MUST NOT declare bindings that fire against a row no user can
see — the same rule the host already applies when it refuses to publish a binding it
could not deliver.

"The row the user is looking at" MUST be read as the row the **pane receiving the
key** draws its cursor on, which for a plugin pane is the plugin's own cursor. A
plugin holding input receives keys only while one of its own panes holds focus, and a
VM persists across renders, so a cursor is ordinary plugin state and needs no view
write. A port MUST NOT conclude that a pane's keys are unportable merely because the
kernel's published cursor is elsewhere; it must ask whether the pane can hold a cursor
of its own and whether every one of the keys has an effect the capability vocabulary
can express.

For the tasks pane the answer remains that it cannot, and the record MUST name the
two independent reasons — each stated against the **kernel's** cursor, which is what
that port's plugin reads because it declared no input:

- **A cursor is view state and nothing writes it.** Moving the *kernel's* selection,
  scrolling the central pane's preview, focusing the editor and switching to a
  related session are all writes to what the user is looking at elsewhere in the
  interface, and the kernel-state channel is read-only by construction. A capability
  that changes a *record* is not this write, which is a distinction an earlier verdict
  already had to be corrected on.
- **The input path and the kernel's cursor path are disjoint.** A plugin receives keys
  only while one of its own panes holds focus, and the published task section marks
  the cursor's row only while the *native* pane holds focus or a search preview is
  moving it. So a plugin acting on the kernel's marked row would act on no row at all.

The pane's two separate surfaces MUST be recorded as kernel-owned with their
reasons rather than left as future work: the central-pane editor is a seat the
pane-slot vocabulary does not offer, a focus a plugin cannot take, and the text
authoring the task-write capability is defined to exclude; the trigger-time
action picker is a modal whose outcomes — typing into a running session and
spawning a new one — are wider than the widest capability the host defines.

#### Scenario: The port ships the rendering and not the keys

- **WHEN** the tasks pane's plugin is inspected after the port
- **THEN** it declares no input capability and no keybinding, and the native
  pane's keys still do what they did

#### Scenario: A key that could act cannot name its row

- **WHEN** a plugin pane holds focus and a status change would be permitted by
  the record-write capability
- **THEN** the published task section marks no row as the cursor's, so no row is
  addressable as "the selected one"

#### Scenario: The separate surfaces are recorded, not attempted

- **WHEN** the phase's pane-readiness audit is read
- **THEN** the central-pane editor and the trigger-time picker are recorded as
  kernel-owned, each with the host powers a plugin would need

#### Scenario: A pane that can hold its own cursor is not refused by this rule

- **WHEN** a later port's pane can keep a cursor inside its VM and every one of its
  keys has an effect the capability vocabulary expresses
- **THEN** that pane's keys are portable, and the rule refuses only the keys whose
  effect the vocabulary cannot express
