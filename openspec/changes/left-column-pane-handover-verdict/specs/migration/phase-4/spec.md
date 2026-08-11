# phase-4 (delta)

## ADDED Requirements

### Requirement: A refused handover records what it needed, re-derived from the source

An attempt to hand a native pane over to its bundled plugin that concludes the
native pane stays SHALL record every requirement it could not meet as a gate that
re-derives each one from the source tree, not as prose alone.

Each row SHALL name the pane behaviour that needs it, where the host stands
today, and **why** it is missing, using at least these kinds:

- *structural* — a power a plugin is not given on purpose, whose reversal changes
  what a plugin is;
- *vocabulary* — something the drawing catalogue cannot say and could;
- *wiring* — something the host could do today with no new plugin-facing concept,
  such as when a plugin is asked to render, or which facts it is told.

The kinds MUST be distinguished, because a verdict that records everything as
structural overstates the wall and one that records everything as vocabulary
misfiles a host decision as a drawing gap — and the ordering of the work that
would unblock the handover follows from the kinds.

The gate SHALL derive the verdict from its rows rather than stating it, so that a
row becoming satisfied changes the answer, and MUST fail when a recorded row no
longer matches the tree.

#### Scenario: A handover attempt is refused

- **WHEN** an attempt concludes that a native pane stays
- **THEN** a gate records one row per unmet requirement, each re-derived from the
  source, and the pane's teardown row stays blocked

#### Scenario: A recorded requirement is met by an unrelated change

- **WHEN** the host gains a power a recorded row says is missing
- **THEN** the gate fails, naming the row, so the verdict is revisited rather than
  quietly expiring

#### Scenario: The verdict follows from the rows

- **WHEN** every row of a pane's gate is satisfied
- **THEN** the derived verdict is that the handover is possible, and the gate says
  so in both directions

### Requirement: A pane whose keys are scoped kernel actions cannot be handed to a plugin pane while focus silences them

A pane whose keyboard is a set of scoped kernel actions SHALL NOT be handed over
while a focused plugin pane resolves keys in the global scope only.

Handing such a pane over moves its focus to the plugin-pane focus, at which point
the pane's own scope never activates and every one of its scoped actions stops
resolving. A plugin MAY declare pane-addressed bindings of its own, so the
handover is possible **only** for keys whose whole effect the plugin can also
perform; a key whose effect is a write to kernel view state — the active session,
a focus, a cursor the rest of the interface follows — has no such substitute, and
a pane handed over with those keys silently loses them.

A pane whose keys write **persisted** state the mutation seam does not address is
likewise not handed over, and a capability MUST NOT be added for it before a
consumer can use it: a grant whose key still cannot name the row it acts on adds
reach without adding parity.

#### Scenario: A pane's scoped keys are silenced by the handover

- **WHEN** a pane whose keys are scoped kernel actions would be drawn by a plugin
  pane
- **THEN** the verdict records that those actions no longer resolve, and the pane
  is not handed over

#### Scenario: A key writes kernel view state

- **WHEN** a pane's key moves a cursor or focus the rest of the interface follows
- **THEN** no plugin binding can reproduce it, and the requirement is recorded as
  structural rather than closed by a new capability

#### Scenario: A key writes persisted state the seam does not address

- **WHEN** a pane's key mutates a record kind the mutation seam has no operation
  for
- **THEN** the missing operation is recorded, and it is not added until a pane
  that could use it exists

### Requirement: A pane whose focus drives another region cannot be handed over until that region is reachable

A pane whose **focus** selects what a different region of the interface displays
SHALL NOT be handed over to a bundled plugin while that selection is made by
testing the native pane's own focus.

Handing such a pane over moves its focus to the plugin-pane focus, which the
selecting branch does not name, so the driven region silently reverts to its
default and every surface reachable only through it disappears — while the pane
itself still draws correctly and its ported keys still work. The shortfall is
therefore a **seat**, not a key: counting unported keys understates it, because the
keys that open the driven region are the ones that cannot be ported, and what they
open is not the pane.

The verdict SHALL record this as a structural requirement, and MUST fail if the
selecting branch begins to name a plugin pane, so that granting a plugin the driven
region is a decision taken deliberately rather than inherited.

#### Scenario: A pane's focus selects the central region

- **WHEN** a native pane's focus is what makes another region show that pane's
  editor or history
- **THEN** the verdict records that a plugin pane cannot make that selection, and
  the pane is not handed over

#### Scenario: Counting ported keys understates the shortfall

- **WHEN** most of a pane's keys are ported and the remainder open the driven region
- **THEN** the verdict records the missing seat rather than the missing keys, and
  names what the handover would have removed

#### Scenario: The driven region becomes reachable

- **WHEN** the selecting branch names a plugin pane
- **THEN** the gate fails, so the widened reach is re-verdicted rather than assumed
