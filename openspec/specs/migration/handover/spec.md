# migration/handover Specification

## Purpose
TBD - created by archiving change info-panel-handover. Update Purpose after archive.
## Requirements
### Requirement: A pane is handed over by deleting its native renderer

A handover SHALL delete the pane's native renderer, not disable it. After a
handover the module MUST NOT exist, no call to it MUST remain, and no
compile-time or runtime switch MUST select between two renderings of the pane.

Keeping the native renderer behind a flag would leave two panes that differ by
build rather than one pane, and it hands nothing over: the deleted call site is
what makes the plugin the pane.

The kernel's own occupant of the handed-over seat SHALL be deleted with it. A
retained visibility flag that no longer decides what is drawn is worse than a
retained renderer, because it can still carve the seat — producing a bordered
column nothing paints, which is the outcome the teardown inventory exists to
prevent.

#### Scenario: The native module is gone

- **WHEN** the tree is searched for the handed-over pane's renderer
- **THEN** neither the module nor any reference to it exists

#### Scenario: The seat is carved only by a claim

- **WHEN** no plugin pane claims the handed-over pane's seat
- **THEN** the seat is not placed and the space goes where it went before the
  seat existed, rather than being carved and left unpainted

#### Scenario: The pane's action reports that nothing provides it

- **WHEN** the action that used to toggle the native pane fires and no pane claims
  its seat — because the plugin failed to load, or the build has no plugin host
- **THEN** the action names what provides the pane instead of appearing to do
  nothing

### Requirement: A handed-over pane keeps the pane's identity

The replacement SHALL be reachable, gated and placed exactly as the native pane
was. Its manifest MUST bind the action that toggled the native pane and the
`[features]` switch that gated it, and it MUST occupy the seat the native pane
occupied — so the width rules, the share of the screen and the toggle a user
already knows are unchanged.

The replacement's title SHALL be the native pane's title. A title that marks the
pane as a plugin is right for a reproduction drawn beside the original and wrong
for the pane itself.

The seed visibility SHALL be the visibility the native pane defaulted to. A
handover changes which code draws a pane, not whether the pane is on screen.

#### Scenario: The pane answers the action it always answered

- **WHEN** the action that toggled the native pane fires
- **THEN** the replacement pane is shown, in the seat the native pane occupied,
  and firing it again hides it

#### Scenario: The feature switch still gates the pane

- **WHEN** the `[features]` switch that gated the native pane is turned off
- **THEN** the replacement is not shown, occupies no seat and is not rendered, and
  turning the switch back on restores the visibility the user last chose

#### Scenario: The pane's own width rule is unchanged

- **WHEN** the terminal is narrower than the width at which the native pane
  appeared
- **THEN** the seat is not placed, exactly as before

### Requirement: A handover's evidence is the recording, not the builder it deletes

A handover SHALL rewrite the pane's oracle against the checked-in recording of the
native pane's tree, and the recordings MUST NOT be regenerated in the same change.
An assertion regenerated from the replacement is a recording of the replacement, so
the deletion would leave the pane constrained by itself.

The oracle MUST still be able to fail for the reason it exists: after the handover
it MUST compare the replacement's tree against the recording, not merely assert
that the replacement renders.

#### Scenario: The recordings are unchanged by the handover

- **WHEN** the change that deletes the native renderer is reviewed
- **THEN** the pane's recorded expectations are byte-identical to what they were
  before it, so the baseline is still the native pane's tree

#### Scenario: The rewritten oracle fails on a wrong row

- **WHEN** the replacement is perturbed to draw one row differently
- **THEN** the oracle fails, naming the row

### Requirement: A behavioural difference a handover exposes is decided, not discovered

Where the replacement and the native pane differ in a state no comparison case
covers, the handover SHALL choose the behaviour deliberately, state why, and pin
it with a test. It MUST NOT be left to be found after the deletion.

An empty state is the case this rule exists for: comparison cases populate the
pane, so a state in which the native pane drew nothing at all is exactly the state
no oracle constrains.

#### Scenario: The empty state is pinned

- **WHEN** the handed-over pane is shown with none of the state it describes
  present
- **THEN** what it draws is asserted by a test, and the reason that behaviour was
  preferred over the native pane's is recorded

### Requirement: A build without the plugin host loses a handed-over pane, and says so

A build configuration that excludes the plugin runtime SHALL lose a handed-over
pane rather than degrade the interface. It MUST NOT carve the pane's seat, MUST NOT
leave the pane's action silent, and MUST NOT keep collecting state only that pane
read without a reader.

The loss MUST be stated rather than discovered: the runtime is part of the default
feature set, so no installed binary is in this configuration, and a change that
removed it from the default set would remove the pane from every install — which
the teardown inventory already fails on.

#### Scenario: No empty seat without the host

- **WHEN** the binary is built without the plugin runtime and the handed-over
  pane's action fires
- **THEN** no seat is carved, nothing is drawn where the pane was, and the action
  reports that no pane provides it

### Requirement: A handed-over pane with a keyboard keeps that keyboard

A pane whose native counterpart had a scoped keyboard SHALL keep it: its manifest
MUST declare the key context the native pane was scoped to, so every action of that
context still resolves while the pane holds focus, still fires against the kernel's
own state, and is still rebindable in the keybinding editor and persisted to the
user's keybindings file.

The keyboard MUST NOT be re-implemented in the plugin. A pane's keys operate on
kernel state — a cursor, a record, a directory listing, an editor process — and
delivering them to a plugin would require granting each of those as a capability,
which is a wider surface than the pane needs and a different pane's behaviour than
the one being replaced.

The pane MUST be focusable through the same entry as the native pane: the focus
cycle stop, or the hand-off keys of the column it sits in, available whenever the
pane is on screen rather than depending on which code draws it.

#### Scenario: A scoped key still works after handover

- **WHEN** a key bound to one of the replaced pane's scoped actions is pressed while
  the replacement holds focus
- **THEN** the action fires exactly as it did before the handover

#### Scenario: The keyboard is still rebindable

- **WHEN** a user rebinds one of that context's actions in the keybinding editor
- **THEN** the new chord drives the replacement pane, and the change is persisted

#### Scenario: Focus reaches the replacement

- **WHEN** the focus cycle is stepped while the replacement is on screen
- **THEN** focus lands on it, reported as the pane it replaced

### Requirement: Chrome a plugin cannot draw stays the kernel's, inside the seat

A handed-over pane MAY have chrome the plugin has no way to draw — a row of key hints
naming **rebindable kernel chords**, an input bar whose text is kernel state. That
chrome SHALL keep being drawn by the kernel, in the seat, in the position it had; the
plugin's tree is laid out in what remains, which is the same area the native pane laid
its own content out in.

It MUST NOT be published to the plugin instead. A chord is a user's setting the kernel
resolves, and a pane redrawing it from published state would be a second renderer for
one fact — while a plugin *inventing* the hint would print a chord the user may have
rebound.

The chrome MUST appear under the same condition it appeared before the handover, and
the pane's content area MUST be the area it had, so a handover changes which code
draws the pane's content and nothing else about the pane.

#### Scenario: The hint row survives the handover

- **WHEN** a handed-over pane whose native counterpart drew a key-hint row holds focus
- **THEN** the row is drawn in the same position, and the plugin's tree occupies the
  rest of the seat

#### Scenario: The chrome follows its own condition

- **WHEN** that pane does not hold focus
- **THEN** the row is not drawn and the plugin's tree occupies the whole seat

### Requirement: A refused handover records what it still needs, and what it does not

A handover that is proposed and refused SHALL record, as executable rows rather than
prose, one entry per thing it still needs — each re-derived from the source so it cannot
go stale.

A row whose requirement has **stopped being a requirement** MUST be re-verdicted rather
than deleted, and MUST keep asserting whatever half of it still matters. In particular,
where a handover was expected to need a **capability** and no longer does, the row MUST
assert that the capability is still **not** granted: otherwise the record of "the grant
was unnecessary" is indistinguishable from the grant having quietly happened.

The refusal MUST distinguish an unmade **decision** from a refused one. A decision the
host declines in principle blocks the pane; a decision nobody has taken yet blocks the
change, and a reader who cannot tell which is looking at the wrong problem.

#### Scenario: A requirement that stopped being one

- **WHEN** a route is added that makes a recorded requirement unnecessary
- **THEN** its row is re-verdicted with a probe deriving the new fact, and still asserts
  that the power it named was not granted

#### Scenario: The remainder is characterised

- **WHEN** the refusal is recorded
- **THEN** a rule asserts what kind of thing is outstanding, so "it needs a capability"
  cannot be inferred from a table where none of the rows is one

