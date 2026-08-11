# migration/handover Specification

## MODIFIED Requirements

### Requirement: A handed-over pane keeps the pane's identity

The replacement SHALL be reachable, gated and placed exactly as the native pane
was. Its manifest MUST bind the `[features]` switch that gated the native pane and it
MUST occupy the seat the native pane occupied — so the width rules, the share of the
screen and the toggle a user already knows are unchanged.

Where the native pane was toggled by an action, the manifest MUST bind that action.
Where the native pane had **no** toggle — it was always on screen while its feature was
on — the manifest MUST bind none, because there is no key whose meaning the pane would
be taking over.

The replacement's title SHALL be the native pane's title. A title that marks the
pane as a plugin is right for a reproduction drawn beside the original and wrong
for the pane itself.

The seed visibility SHALL be the visibility the native pane defaulted to. A
handover changes which code draws a pane, not whether the pane is on screen. So a pane
replacing an always-visible one seeds **visible**, and a rule binding handed-over panes
MUST compare against the native default rather than requiring one particular value —
otherwise it forbids the correct seed for such a pane while permitting a hidden seed for
a pane users expect to see.

A pane that seeds **hidden** MUST bind an action, or nothing could reveal it. A pane
that seeds visible need not.

#### Scenario: The pane answers the action it always answered

- **WHEN** the action that toggled the native pane fires
- **THEN** the replacement pane is shown, in the seat the native pane occupied,
  and firing it again hides it

#### Scenario: A pane that was never toggled binds no action

- **WHEN** the native pane had no toggle action and was always on screen while its
  feature was on
- **THEN** the replacement binds no action, seeds visible, and is on screen without a
  keystroke

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

An edge of the oracle that is **not** differential against the deleted builder SHALL be
kept rather than dropped with it. Where the pane's reproduction is held to a *rule* that
outlives the deletion — a formatter a surviving surface also composes — that comparison
is the only one that constrains behaviour a recording cannot enumerate, so a handover
that removed it would trade an exhaustive claim for a fixed set of cases. Deciding which
edges go therefore means asking of each one whether its right-hand side is being
deleted, not whether the change is a handover.

#### Scenario: The recordings are unchanged by the handover

- **WHEN** the change that deletes the native renderer is reviewed
- **THEN** the pane's recorded expectations are byte-identical to what they were
  before it, so the baseline is still the native pane's tree

#### Scenario: The rewritten oracle fails on a wrong row

- **WHEN** the replacement is perturbed to draw one row differently
- **THEN** the oracle fails, naming the row

#### Scenario: A surviving rule keeps its edge

- **WHEN** an edge of the oracle compares the replacement against a formatter the
  handover does not delete
- **THEN** that edge is kept, and the change records that its right-hand side survives

## ADDED Requirements

### Requirement: A handed-over pane takes the kernel's keyboard rather than keeping the plugin's

Where a pane's reproduction was built to hold its **own** keys — an input capability
plus pane-addressed bindings, acting through granted write operations — the handover
SHALL move it onto the kernel keyboard instead: the pane declares that it *is* the
interface's pane for that key context, and the kernel resolves and performs that
context's actions while the pane holds focus.

The plugin's own bindings and the write capabilities they used MUST be **removed** from
the manifest in the same change. A pane declaring both routes is refused, so this is a
fork rather than a supplement; and a pane keeping capabilities it no longer exercises
would leave an installed plugin holding reach nothing uses.

A handover MUST NOT be made reachable by widening the write seam instead. Where a
pane's remaining keys create a record or author a field, those operations are refused on
their own merits, and a pane granted them would still lose any surface its focus opens —
so two new grants would buy a strictly worse outcome than the declaration.

A handover MUST NOT be made reachable by teaching the surfaces a focus drives to
recognise a plugin pane. Reusing the kernel's own focus is what leaves the key-context
resolver, the focus ring, the return paths from any editor the pane opens, and the
escape key untouched; a second mechanism for the same fact is how a handed-over pane
comes to *almost* work.

The reduction in the plugin's reach SHALL be recorded as the finding rather than as a
cost. A pane whose keys looked like they needed the widest grants needing the fewest is
the result the route exists to produce.

#### Scenario: The route is switched and the grants are dropped

- **WHEN** a pane whose reproduction held its own keys is handed over
- **THEN** its manifest declares the kernel key context, declares no bindings of its
  own, and no longer declares the input or write capabilities those bindings used

#### Scenario: Every scoped action still resolves

- **WHEN** the handed-over pane holds focus
- **THEN** every action scoped to that key context resolves and is performed by the
  kernel against its own state, including the ones no write operation could express,
  and each remains rebindable

#### Scenario: Widening the seam is refused

- **WHEN** closing the pane's remaining keys by adding a creation or field-writing
  operation is proposed
- **THEN** it is refused, and the refusal records that the pane would still lose the
  surfaces its focus opens

### Requirement: A handed-over pane that was always on screen arrives after the first frame

The plugin host starts detached and the first frame does not wait for it, so a pane does
not exist until the host publishes. A handover of a pane that was **always visible**
therefore SHALL be recorded as producing a visible layout change on launch: the seat is
not carved, the space goes where it went before the seat existed, and the pane appears
when the host arrives.

The seat MUST NOT be carved from the pane's feature flag to avoid that change. A flag
nobody paints from carves a band that stays blank whenever the pane is absent for any
other reason — a plugin that failed to load, a manifest that declares no such pane, or a
build with no host — which is the empty-column outcome the teardown inventory exists to
prevent.

The first frame MUST NOT be made to wait for the host. The kernel never calls a plugin
during a frame, and a slow or wedged plugin must not delay the interface.

The residual cost SHALL be named rather than absorbed, and attributed to **startup**
rather than to the pane: how soon the host publishes is the same question for every pane
that follows.

#### Scenario: The band is absent until the host publishes

- **WHEN** the interface draws its first frame and no plugin pane has been published yet
- **THEN** the handed-over pane's seat is not carved and the column's other occupant
  takes the space

#### Scenario: The pane appears without a keystroke

- **WHEN** the host publishes a visible pane claiming that seat
- **THEN** the seat is carved and the pane is drawn, with no user action

#### Scenario: Carving from the flag is refused

- **WHEN** retaining the feature flag as the seat's condition is proposed to avoid the
  reflow
- **THEN** it is refused, because the band would stay blank in every case where the pane
  is absent for another reason
