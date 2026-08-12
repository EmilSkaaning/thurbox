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
that owns it, and the relocation MUST NOT change behaviour.

The relocation SHALL happen in the handover's own change, **except** where the handover is
refused on a requirement the relocation does not close. In that case the relocation SHALL
be hoisted into a change of its own, ahead of the handover, for the reason a pane's
keyboard is hoisted: a change that both relocates a model and moves who draws a pane makes
any behavioural difference read equally as either, and a model stranded in a rendering
layer by an unrelated refusal is stranded for as long as that refusal stands. A hoisted
relocation MUST state that it hands no pane over, and the pane's gate row for the
relocation MUST be re-verdicted by it while every other row keeps its verdict.

The pane's model belongs to whatever already owns its value. A model that performs
side effects — reading directories, launching a process — MUST NOT be relocated into a
layer the architecture keeps free of them, however well its *types* would fit there: a
pure-data layer holding I/O is a worse outcome than a coordinator holding a state
machine. Correspondingly, a model that performs **no** effects and is a pure function of
data that layer already owns SHALL be relocated into it, rather than into the coordinator,
so that the two cases are decided by the same rule rather than by which came first.

A part of the model that is **downstream of a seam the refusal has not settled** SHALL NOT
be relocated with the rest, and the exclusion MUST be stated. Relocating it would fix its
home against a seam that does not exist yet.

Geometry SHALL NOT cross into the pure-data layer with the model. Where the model and a
width-dependent fit share a type, the type MAY move while the fit stays, provided the
moved producer leaves the geometry-bearing field unset.

A shared helper MUST move to the layer's own shared vocabulary rather than to one of its
callers, so that no surface windows a list by a rule that lives in a different pane's
module.

Where the relocation leaves two types with the same fields and one producer, the
duplicate SHALL be **deleted** rather than carried: a handover is the moment the second
one stops having a reason to exist.

A relocation MUST NOT be satisfied by re-exporting the moved items from the module they
left. The re-export preserves exactly what the relocation exists to remove — the module
remains the name the kernel calls its own model by, and the handover's deletion problem is
unchanged.

#### Scenario: The deleted module was also the model

- **WHEN** a pane's renderer is deleted and its state machine lived in the same module
- **THEN** the state machine is relocated to the layer that owns the value, unchanged,
  and the pane behaves identically

#### Scenario: The model performs side effects

- **WHEN** relocating that state machine into the pure-data layer is proposed because its
  types would fit
- **THEN** it is refused, because the model reads the filesystem and that layer is kept
  free of effects

#### Scenario: The model performs no side effects

- **WHEN** the model is a pure function of data the pure-data layer already owns
- **THEN** it is relocated into that layer rather than into the coordinator, by the same
  rule that refused the previous case

#### Scenario: The handover is refused on an unrelated requirement

- **WHEN** a pane's model sits in its renderer and the pane's handover is blocked by a
  requirement the relocation does not close
- **THEN** the relocation is done in its own change ahead of the handover, that change
  hands no pane over, and only the relocation's own gate row is re-verdicted

#### Scenario: Part of the model is downstream of an unsettled seam

- **WHEN** one function of the model is consumed by the very seam the refusal is about
- **THEN** it stays where it is, and the change states why it was excluded

#### Scenario: The relocation is proposed as a re-export

- **WHEN** the moved items are re-exported from their old module so no caller changes
- **THEN** it is refused, because the kernel would still name its own model through the
  rendering layer and the module would still be undeletable

#### Scenario: A shared helper outlives the module

- **WHEN** the deleted module held a helper other surfaces call
- **THEN** the helper moves to the layer's shared vocabulary and every caller is updated
  in the same change

#### Scenario: The relocation exposes a duplicate type

- **WHEN** the relocated model's row type has the same fields as the published row type
  and the publication is now its only consumer
- **THEN** one of the two is deleted rather than both being kept

### Requirement: A pane's keyboard becomes rebindable kernel actions before its handover

A native pane whose keys are a **capture** — a handler keyed on the focused surface and
run ahead of the keybinding lookup — SHALL have those keys turned into scoped kernel
actions in a change **before** its handover, and MUST NOT have them turned into actions
inside it.

The reason is the one the frame convergence rule states: a handover asserts that which
code draws a pane changed and nothing else did. A commit that both rewrites a keyboard and
moves who paints a pane makes a missing key unattributable — it reads equally as a
keyboard mistake and a handover regression — and the F1 editor indexes its selection into
the flattened help sections, so the rows move for two reasons at once.

The keys MUST become **rebindable**: each declared as an action with a key context, listed
by the help overlay, and writable to the keymap file. A key left literal is a key no
handed-over pane can be given back, because the declaration a pane makes names a context,
not a keystroke.

Where the capture **shadowed** a global chord, the shadowing SHALL NOT be reproduced as a
default binding. A scoped default that collides with a global one is a conflict the keymap
reports as a defect, so the action takes an unshadowed default and the user may rebind it.

Where the capture **swallowed** chords it did not itself act on, the swallowing SHALL be
removed rather than reproduced: after the change the pane resolves keys the way every
other pane does — a scoped action wins, otherwise the global one fires. A per-pane
allowlist of which global keys work is the inconsistency a context lookup exists to
remove.

A **sub-mode that owns every key** — a text field, a picker overlay — MAY remain a capture,
and MUST remain one where a letter typed into it is text rather than a command.

#### Scenario: A captured keyboard is declared before the handover

- **WHEN** a native pane whose keys are a focus-keyed capture is prepared for handover
- **THEN** its keys become scoped actions in their own change, and the handover changes no
  keyboard

#### Scenario: Every declared key is rebindable

- **WHEN** the keys become actions
- **THEN** each appears in the help overlay's editable rows and can be rebound and
  persisted, and no key of the pane's own keyboard is left literal

#### Scenario: A shadowed global chord is not reproduced

- **WHEN** the capture bound a chord a global action already holds
- **THEN** the declared action takes a default that collides with nothing, and the change
  records the moved chord as a decided difference

#### Scenario: A text sub-mode stays a capture

- **WHEN** a sub-mode of the pane owns every key while it is open
- **THEN** it stays captured ahead of the lookup, and its keys are recorded as not
  rebindable

### Requirement: A capability row closes when the kernel keeps the key, and still asserts the grant was not made

Where a refused handover recorded a row naming a **power no capability performs**, and the
pane's keys later become kernel actions, that row SHALL be re-verdicted as met — because
the kernel performs the power itself while the pane holds focus, against its own state,
and the pane is told nothing.

The re-verdicted row MUST keep asserting that the capability it named is **still not
granted**, and MUST derive the new verdict from the source: that the action exists, that it
is scoped to the pane's key context, and that the kernel dispatches it.

A row MUST NOT be re-verdicted where the kernel's performance of it depends on a surface
the pane cannot host. A key that opens an overlay anchored to a row of a tree the kernel
did not lay out is not performed by giving the kernel the key.

#### Scenario: A power row closes without a grant

- **WHEN** a key that mutates a record becomes a scoped action the kernel dispatches
- **THEN** the row naming the missing write capability is re-verdicted met, and asserts
  that no such capability exists

#### Scenario: A power whose surface the pane cannot host stays blocked

- **WHEN** the key opens a surface anchored inside the pane's own content
- **THEN** the row stays blocked, and names the surface rather than the power

#### Scenario: The remaining refusal is still derived

- **WHEN** rows close
- **THEN** the verdict is still computed from the rows, and the handover is still refused
  by the ones that did not

### Requirement: A pane's window is converged before its handover, not during it

Where a native pane resolves which rows are on screen by a rule other than the kernel's
own, that window SHALL be converged in a change **before** its handover, and MUST NOT be
converged inside it.

The reason is what a handover is allowed to claim: that which code draws a pane changed
and nothing else about the pane did. A commit that also changes how the pane scrolls makes
that claim unverifiable, because every moved cell has two candidate causes and the
recorded expectation moves for two reasons at once. This is the frame rule applied to the
window — a window, like a frame, is a property of how the host draws a pane, and a
handover must not be able to change one under cover of moving the drawing code.

Convergence MUST run in the direction of the kernel's rule, never the other way.

The visible consequence SHALL be recorded as a decided behavioural change with its reason
— which rows are beside the cursor when the list overflows — rather than left to be
discovered in a frame diff.

#### Scenario: The window converges first

- **WHEN** a native pane whose window is its widget's is prepared for handover
- **THEN** the pane is changed to window by the kernel's rule in its own change, and the
  handover changes no window

#### Scenario: The changed scrolling is stated

- **WHEN** convergence changes which rows sit beside the cursor in an overflowing list
- **THEN** the change states that consequence and why the kernel's rule is the one both
  panes take, rather than leaving it to a diff

#### Scenario: Converging the rule to the pane is refused

- **WHEN** closing the difference by changing the kernel's helper to the pane's widget
  behaviour is proposed
- **THEN** it is refused, because the helper is shared and the pane is the thing being
  deleted

