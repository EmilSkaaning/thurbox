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
rebound. The same holds for an input bar: the kernel owns the key that opens it, so it
owns the query, the caret and the match count, and publishing them to be redrawn would
add state to a capability that deliberately withholds it.

Chrome MUST NOT be restricted to a single row. Where the native pane drew a bordered,
multi-row band — outside its own frame rather than inside it — the kernel SHALL draw the
same band in the same place, subtracting it from the seat before the pane's frame is
drawn.

Chrome SHALL be described to the seat as **data**, in a closed set of shapes, rather than
as a painter the seat invokes: a painter argument would make "the kernel draws whatever it
likes inside a plugin pane" the rule, where the point is that what a seat may draw stays
enumerable.

The chrome MUST appear under the same condition it appeared before the handover, and
the pane's content area MUST be the area it had, so a handover changes which code
draws the pane's content and nothing else about the pane. Different chrome MAY have
different conditions — a hint row that follows focus and an input bar that follows its
own sub-mode — and each MUST keep the condition its native counterpart had.

#### Scenario: The hint row survives the handover

- **WHEN** a handed-over pane whose native counterpart drew a key-hint row holds focus
- **THEN** the row is drawn in the same position, and the plugin's tree occupies the
  rest of the seat

#### Scenario: The chrome follows its own condition

- **WHEN** that pane does not hold focus
- **THEN** the row is not drawn and the plugin's tree occupies the whole seat

#### Scenario: The chrome is an input bar the native pane drew below its frame

- **WHEN** a handed-over pane's search sub-mode is active or its query is committed
- **THEN** the kernel draws the same bordered bar, in the same rows, with the query, the
  caret and the match count it always showed, and the pane's frame occupies the rest of
  the seat

#### Scenario: Publishing the bar's state instead is proposed

- **WHEN** it is proposed that the plugin draw the bar from published state
- **THEN** it is refused, because the query is the kernel's and the capability publishes
  no query

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

### Requirement: A pane's frame is converged before its handover, not during it

The host draws one frame around every plugin pane, whatever seat it holds: a seat
decides *where* a pane is drawn and never *how*. So a native pane whose own frame
differs from that one — different border corners, a different focused colour, a
different title style, or a focus level it collapses into another — SHALL be converged
onto the host's frame in a change **before** its handover, and MUST NOT be converged
inside it.

The reason is what a handover is allowed to claim. A handover asserts that which code
draws a pane changed and nothing else about the pane did; a commit that also restyles
the border makes that claim unverifiable, because a reviewer cannot separate the
intended restyle from a regression and the frame snapshot moves for two reasons at
once.

Convergence MUST run in the direction of the host's frame, never the other way: the
manifest MUST NOT grow border or title options so a pane can bring its own frame. A
plugin-declared frame would let a pane draw itself as focused when it is not, which is
the confusion resolving the level from the kernel's own focus exists to prevent.

Where convergence starts drawing a focus level the native pane was collapsing, that
consequence SHALL be recorded as a visible change with its reason, rather than left to
be discovered in a frame diff.

#### Scenario: The frame converges first

- **WHEN** a native pane whose frame differs from the host's is prepared for handover
- **THEN** the native pane is changed to draw the host's frame in its own change, and
  the handover changes no border

#### Scenario: A plugin cannot bring its own frame

- **WHEN** closing the difference from the plugin's side is proposed
- **THEN** it is refused, because the frame is the host's and a pane's focus is the
  kernel's to resolve

#### Scenario: A collapsed focus level starts being drawn

- **WHEN** the host's frame distinguishes a level the native pane drew as another
- **THEN** the change records that the pane now draws it, and why that reading is the
  correct one

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

### Requirement: A handover relocates the model the deleted module also held

Where the module a handover deletes holds more than the pane's renderer — the pane's
**model**, or a helper other surfaces share — each part SHALL be relocated to the layer
that owns it, in the same change, and the relocation MUST NOT change behaviour.

The pane's model belongs to whatever already owns its value. A model that performs
side effects — reading directories, launching a process — MUST NOT be relocated into a
layer the architecture keeps free of them, however well its *types* would fit there: a
pure-data layer holding I/O is a worse outcome than a coordinator holding a state
machine.

A shared helper MUST move to the layer's own shared vocabulary rather than to one of its
callers, so that no surface windows a list by a rule that lives in a different pane's
module.

Where the relocation leaves two types with the same fields and one producer, the
duplicate SHALL be **deleted** rather than carried: a handover is the moment the second
one stops having a reason to exist.

#### Scenario: The deleted module was also the model

- **WHEN** a pane's renderer is deleted and its state machine lived in the same module
- **THEN** the state machine is relocated to the layer that owns the value, unchanged,
  and the pane behaves identically

#### Scenario: The model performs side effects

- **WHEN** relocating that state machine into the pure-data layer is proposed because its
  types would fit
- **THEN** it is refused, because the model reads the filesystem and that layer is kept
  free of effects

#### Scenario: A shared helper outlives the module

- **WHEN** the deleted module held a helper other surfaces call
- **THEN** the helper moves to the layer's shared vocabulary and every caller is updated
  in the same change

#### Scenario: The relocation exposes a duplicate type

- **WHEN** the relocated model's row type has the same fields as the published row type
  and the publication is now its only consumer
- **THEN** one of the two is deleted rather than both being kept

