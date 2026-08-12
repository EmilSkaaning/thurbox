# migration/phase-4 Specification

## Purpose
Defines what porting one of thurbox's own panes to a bundled plugin must
demonstrate. Phase 0 proved the view tree can *express* a real pane; this phase
asks whether a plugin can *obtain* what it draws, using only surface a third
party could declare. So each port is measured two ways: the plugin must produce
the native pane's view tree exactly, and every gap in the host surface it hit
must be closed or recorded — a gap worked around inside a bundled plugin proves
nothing, since the point of shipping one is to measure what an outsider gets. The
port is additive: the native pane keeps drawing until a later phase hands over.
## Requirements
### Requirement: A native pane is reproduced by a bundled plugin

At least one of thurbox's own panes SHALL be reproduced by a plugin shipped
inside the binary, written in the same language and against the same host surface
a third party would use. The plugin MUST obtain everything it draws through
declared capabilities — no private binding, no argument the host passes only to
bundled plugins.

The chosen pane is the **info panel**, because Phase 0 established that the view
tree can express it, which isolates this phase's question to whether a plugin can
*obtain* what it draws.

#### Scenario: The plugin ships inside the binary

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the pane's plugin is discoverable, and its manifest satisfies the same
  validation a user's plugin does

#### Scenario: The plugin uses no privileged surface

- **WHEN** the plugin's manifest is read
- **THEN** every host power it uses is a declared capability, and a user plugin
  declaring the same set would receive the same environment

#### Scenario: A user copy overrides the bundled one

- **WHEN** a user places a plugin of the same name in their own plugin directory
- **THEN** theirs is loaded instead, as for any bundled plugin

### Requirement: The ported pane produces the native pane's view tree

The plugin SHALL produce a view tree **equal** to the one the native pane builds
from the same state, across a range of content variants including absent optional
sections. Equality of the tree is the check, because the same renderer paints
both — so an equal tree is a byte-identical pane without needing to compare
frames.

A divergence MUST be enumerated with its reason and pinned by its own test,
never absorbed by weakening the comparison.

**The comparison MUST also be able to outlive the native builder it names.** A
port's equality is written against an expression the eventual handover deletes, so
a port whose pane is a handover candidate SHALL additionally record the native
pane's tree as a checked-in expectation, and assert the plugin against that. A
port MUST NOT rely on the differential assertion alone as its handover evidence,
because the handover removes one side of it and what survives is a test that the
plugin renders without erroring — which a pane drawing entirely wrong rows also
satisfies.

#### Scenario: Trees agree for a fully populated pane

- **WHEN** the native pane and the plugin are given the same state, with every
  optional section present
- **THEN** the two view trees are equal

#### Scenario: Trees agree when optional sections are absent

- **WHEN** the same comparison is run with the optional sections omitted one at a
  time
- **THEN** the two view trees are equal in each case

#### Scenario: A divergence is pinned

- **WHEN** the plugin cannot reproduce some part of the native pane
- **THEN** a test asserts what it does instead and states why, and the
  comparison for every other case still demands equality

#### Scenario: The native builder is deleted by a later handover

- **WHEN** a pane's native builder is removed and only the plugin remains
- **THEN** a recorded expectation still constrains the pane's tree for every case
  the differential comparison covered

### Requirement: The native pane survives the port

The port SHALL be additive. The native renderer MUST stay compiled in and MUST
remain the pane the interface draws by default, and the plugin's pane MUST NOT be
visible until a user asks for it. Replacing the native pane is a later phase,
and the teardown inventory MUST continue to protect the native renderer until
that handover happens.

That later phase has a precondition this phase MUST NOT be read as satisfying: a
pane may stop being drawn natively only when its replacement is present in the
build a user installs. While the plugin runtime is reachable only behind a
compile-time feature that released binaries do not enable, **no port may become a
handover**, however exactly the plugin reproduces the pane — dropping the native
renderer in that state removes the pane from every install while the only build
able to draw the replacement is one nobody runs.

A port MUST NOT satisfy this by keeping both renderers and selecting between them
on the compile-time feature. That leaves two renderings of one pane which differ
by build rather than one pane, and it hands nothing over: the native renderer is
still what the installed binary draws.

**This requirement binds a pane until that pane's handover lands.** It is a rule
about what a *port* may do, and it is discharged for a pane whose replacement has
satisfied every condition the teardown inventory names — at which point the native
renderer is deleted and the plugin's pane is the pane by definition, not by default.

For a pane **with a keyboard**, being discharged additionally requires that the
keyboard survive: the replacement must answer the same scoped actions, against the
same kernel state, still rebindable — which a pane declaring the kernel's key context
does without the plugin being granted anything. A handover that re-implemented those
keys in the plugin would have to be granted every power they exercise, and would be a
different pane rather than the same one.

#### Scenario: The default interface is unchanged

- **WHEN** thurbox starts with a bundled plugin present that reproduces a pane
  thurbox still draws, and no stored visibility choice
- **THEN** the plugin's pane is off screen and the native pane renders as before

#### Scenario: The native renderer is still protected

- **WHEN** the teardown inventory is checked after the port
- **THEN** the native pane's renderer is still required to exist, because the
  pane has not been handed over

#### Scenario: A port is attempted as a handover while the runtime is gated

- **WHEN** a pane's plugin reproduces it exactly and the plugin runtime is absent
  from the default build
- **THEN** the native renderer stays the pane the interface draws, and the
  attempt is recorded with the release decision it waits on rather than landing
  as a handover

#### Scenario: The proof a handover offers is checked for being able to fail

- **WHEN** a handover claims that unchanged rendering snapshots demonstrate the
  replacement is equivalent
- **THEN** at least one of those snapshots must contain the pane, and a handover
  whose snapshots contain none of it MUST state that instead of citing them

#### Scenario: A pane whose handover has landed

- **WHEN** every condition the teardown inventory names is satisfied for a pane and
  its native renderer is deleted
- **THEN** this requirement is discharged for that pane and still binds every other
  reproduced pane, so a second handover is argued on its own evidence rather than on
  the first one's

#### Scenario: A pane whose keyboard is the kernel's is handed over

- **WHEN** a native pane with a scoped keyboard is deleted and its replacement declares
  that key context
- **THEN** every one of that context's actions still fires against the kernel's state,
  and the plugin holds no new capability

### Requirement: The port reports whether the host surface sufficed

Porting a pane SHALL report every host-surface gap it hit, and each MUST either be
closed in the same change or recorded as still open with its reason. A gap worked
around inside the bundled plugin — by a shortcut a third party could not take —
MUST be recorded as still open, because the point of shipping a bundled plugin is
to measure the surface a third party gets.

#### Scenario: A gap closed by widening the surface

- **WHEN** the plugin needed something no binding provided
- **THEN** the binding was added under a declared capability and the
  pane-readiness audit records the gap as closed

#### Scenario: A gap left open

- **WHEN** the ported pane behaves worse than the native one in some respect
- **THEN** the audit records that, with the measurement, so the next pane's port
  does not assume it was settled

### Requirement: A ported pane is reachable from the keyboard

A pane reproduced as a bundled plugin SHALL be reachable from the keyboard
without knowing its plugin's name — a user MUST be able to put it on screen with
the bound pane-visibility action alone. A port MUST NOT rely on a headless
command or a stored choice as the only way to see the pane it added, because a
pane nobody can open is not evidence that the pane was ported.

This holds however many bundled panes exist, so it cannot regress as later panes
are added.

#### Scenario: The newest bundled pane can be shown

- **WHEN** the pane-visibility action is used with every bundled pane declared
- **THEN** each declared pane, including the most recently added one, can be put
  on screen and taken off again

#### Scenario: Reachability does not depend on declaration order

- **WHEN** a second bundled plugin declares a pane after an existing one
- **THEN** the later pane is as reachable as the first

### Requirement: A second native pane is reproduced by a bundled plugin

A second of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, and leaving the native
pane as the one the interface draws.

The chosen pane is the **tasks pane**, because it is the first *list* pane — a
selectable list with search emphasis, which is the shape every remaining Phase 4
pane has — so what it needs is what those ports will need.

#### Scenario: The second pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the tasks pane's plugin is discoverable, its manifest satisfies the
  same validation a user's plugin does, and its pane is off screen until asked
  for

#### Scenario: Both bundled panes coexist

- **WHEN** both bundled panes are put on screen
- **THEN** each renders its own pane, and neither native renderer is replaced

### Requirement: A list pane's row styling is expressible without naming a colour

A pane reproduced as a plugin SHALL be able to draw a selectable list row in
every appearance the native pane gives it — selected, filtered out by a running
search, and with matched characters emphasised — using only the declared style
vocabulary. If it cannot, the vocabulary MUST be widened in the same change
rather than the pane approximating one appearance with another.

#### Scenario: Three row appearances are reproduced exactly

- **WHEN** the native pane and the plugin are given a list containing a selected
  row, a row filtered out by a search, and a row with matched characters
- **THEN** the two view trees are equal, so each row is drawn identically

#### Scenario: The pane still names no colour

- **WHEN** the plugin's rows are inspected
- **THEN** every one is styled by token and emphasis, and none names a colour

### Requirement: A pane whose rows depend on geometry keeps that geometry in the kernel

When a native pane's rows depend on its resolved size — fitting a label to the
column, reserving room for a trailing marker, scrolling a window to keep the
selection visible — the port SHALL leave that resolution in the kernel rather
than reporting a rect into a plugin. The plugin's copy of the pane MUST therefore
be allowed to differ in exactly those respects, and each difference MUST be
pinned by its own test naming what the plugin does instead and what would close
it.

A port MUST NOT hide such a difference by publishing rows already fitted to
another pane's size: the plugin's pane is a different rect, so rows fitted
elsewhere would be wrong at its own size.

**Height is no longer one of those differences.** A ported list pane SHALL
declare the row its cursor is on and let the kernel resolve the window, so the
copy scrolls where the native pane scrolls. What remains kernel-only is
width-dependent *fitting*, because no node clips with an ellipsis.

#### Scenario: Rows fit in the column

- **WHEN** every row fits the width and the list fits the height
- **THEN** the native pane's tree and the plugin's are equal

#### Scenario: A row is wider than the column

- **WHEN** a row's label exceeds the native pane's width
- **THEN** the native pane fits it and the plugin's copy does not, and a test
  asserts that difference and names the node that would remove it

#### Scenario: The list is longer than the pane

- **WHEN** there are more rows than the pane has lines
- **THEN** both panes window to the cursor by the same rule, and a test asserts
  they paint the same frame rather than asserting a difference

### Requirement: A third native pane is reproduced by a bundled plugin

A third of thurbox's own panes SHALL be reproduced by a bundled plugin under the
same rules as the first two: shipped inside the binary, written against declared
capabilities only, producing the native pane's view tree, off screen by default,
and leaving the native pane as the one the interface draws.

The chosen pane is the **file viewer's tree**, because it is the pane that cannot
accept the scrolling gap the previous port recorded: its entire interaction is
moving a cursor through a tree taller than its column, so a copy that draws from
the first row is not a reproduction of it.

#### Scenario: The third pane's plugin ships and loads

- **WHEN** thurbox is installed with nothing downloaded
- **THEN** the file viewer's plugin is discoverable, its manifest satisfies the
  same validation a user's plugin does, and its pane is off screen until asked for

#### Scenario: The reproduction is equal to the native tree

- **WHEN** the native pane and the plugin are given the same tree
- **THEN** the two view trees are equal, across collapsed and expanded
  directories, nested depths, both marker glyph sets, a running search, a selected
  row, and an empty tree

### Requirement: A pane's scroll window is resolved by the kernel from a declared selection

When a ported pane's list is longer than the rows it has, the scroll window SHALL
be resolved by the kernel from a selection the plugin declares — not by reporting
the pane's resolved height into the plugin, and not by publishing rows already
windowed to another pane's size.

The rule that resolves it MUST be the same one thurbox's own panes use, so that a
native pane and the plugin reproducing it are not merely equal as trees but paint
the same frame when the pane scrolls.

This closes the second of the two geometry gaps the previous port recorded, for
every remaining pane rather than only this one.

#### Scenario: The plugin's pane scrolls to its cursor

- **WHEN** the tree has more rows than the pane has lines and the cursor is below
  the fold
- **THEN** the plugin's pane draws the slice containing the cursor, the same slice
  the native pane draws

#### Scenario: The plugin still learns no dimension

- **WHEN** the plugin's rendering is inspected
- **THEN** nothing in it consults a width or a height, and the rows it returns are
  the whole list

### Requirement: A pane sub-mode the host surface cannot express is declared out of scope

When part of a pane cannot be reproduced because the host surface cannot express
it, the port SHALL declare that part out of scope **in its proposal**, naming the
host features that are missing, rather than omitting it silently. The parts of the
same behaviour that *are* expressible MUST still be ported, so the record
distinguishes "cannot be drawn" from "was not attempted".

#### Scenario: The unexpressible part is named with what it needs

- **WHEN** the file viewer's search bar cannot be drawn
- **THEN** the proposal states it is out of scope and the readiness document names
  the missing host features, one per missing capability of the surface

#### Scenario: The expressible part of the same behaviour is still ported

- **WHEN** a search is running
- **THEN** the plugin's tree draws the search's effect on every row — matched and
  unmatched — identically to the native pane, even though the search bar is absent

### Requirement: A scheduled surface that the pane model cannot express is recorded, not approximated

When a surface this phase schedules cannot be reproduced by a plugin pane at all,
the port SHALL produce the record instead of the plugin: every host power the
surface needs and does not have, named individually with the reason it is
missing. No bundled plugin reproducing only the part the host can already express
MUST be shipped, because the phase measures the surface a third party gets and a
pane that cannot do the surface's job reports a capability the host does not have.

The record MUST separate a **vocabulary** gap — one the host would close with a
further node, style token or emphasis — from a **structural** one, where the
surface is not a pane. Only the first is closed by widening the catalogue, and a
change MUST NOT close a vocabulary gap for a pane it is not shipping.

#### Scenario: A surface is assessed as unportable

- **WHEN** a scheduled surface cannot be reproduced by a plugin pane
- **THEN** the change records each missing host power with its reason, ships no
  bundled plugin for that surface, and adds no capability, node, style token or
  pane slot

#### Scenario: The expressible part is not shipped as a gesture

- **WHEN** part of the surface's rendering could be expressed with today's
  catalogue
- **THEN** the record says so, and no pane is shipped that reproduces only that
  part

#### Scenario: The native surface is untouched

- **WHEN** such a change lands
- **THEN** the surface renders exactly as before and the teardown inventory still
  protects its renderer

### Requirement: Global search is recorded as structurally unportable

Global search SHALL be recorded as out of scope for the bundled-plugin phase on
structural grounds, not for want of vocabulary. The record MUST name at least
these four, each of which is a power the pane model withholds by design:

- the layout cannot seat a full-width band for a plugin — the pane-slot
  vocabulary is a closed set whose only member is the right-hand column;
- no capability publishes the query or its results, and none can be scoped
  honestly: computing the search requires reading every session's live terminal
  screen, while publishing the kernel's results would publish the strip's
  rendering rather than kernel state;
- the surface *produces* the restyling of rows in panes it does not own: a
  running search's verdict already reaches a plugin as a property of its own
  published rows, but a plugin's tree is painted into its own rect and nothing
  carries a query the other way;
- activating or previewing a result writes focus and another pane's cursor, and
  the kernel-state channel is read-only by construction.

The record MUST also name the vocabulary gaps separately, and MUST NOT close them
in the same change.

#### Scenario: The verdict is recorded with its reasons

- **WHEN** the phase's pane-readiness audit is read
- **THEN** global search's section states that it cannot be a plugin pane under
  this model and names each structural blocker and each vocabulary gap

#### Scenario: No bundled plugin claims the surface

- **WHEN** the bundled plugins are enumerated
- **THEN** none of them is a global-search pane

### Requirement: An unportability verdict is re-derived from the source

A recorded unportability verdict SHALL be re-derived from the source tree by a
test, so that closing one of its blockers fails the record rather than leaving it
to expire unnoticed. Each blocker MUST have its own probe, a probe MUST be scoped
to the declaration it reads rather than to a whole file, and a failure MUST name
the blocker whose verdict changed.

The verdict MUST NOT be merged into the teardown inventory, which answers a
different question — whether a native renderer may be deleted — and whose verdict
for the surface is unchanged either way.

#### Scenario: A blocker is closed later

- **WHEN** a host change closes one of the recorded blockers
- **THEN** the test fails and names it, so the verdict is revisited in the change
  that closed it

#### Scenario: The verdict still holds

- **WHEN** nothing relevant has changed
- **THEN** the test passes, and it also asserts that no bundled plugin claims the
  surface

#### Scenario: The teardown inventory is unaffected

- **WHEN** the teardown inventory is checked after the record lands
- **THEN** the surface's native renderer is still required to exist, for the same
  reason as before

### Requirement: A pane may be ported in part when its whole is not expressible

When a native pane is too large or too geometry-dependent to reproduce whole, the
port SHALL reproduce a named **core** of it completely, and SHALL itemise
everything left out in its proposal — one entry per omitted behaviour, each with
the reason it could not be drawn.

A partial port MUST NOT approximate what it omits. Drawing a diff without its row
tint, or a header without its rule, would make the reproduction agree with nothing
and the record a claim about a pane that does not exist.

The itemised remainder MUST be re-examined when the port is revisited, and each
entry MUST be classified as either **document** or **behaviour**: an entry is
document when the pane could draw it from published facts alone, and behaviour when
it needs a host power the plugin surface does not have. The classification is what
makes a partial port's remainder actionable rather than a standing list — the
document half is closable by publishing what the kernel already knows, and only the
behaviour half is a decision about the plugin surface.

The chosen core is the code-review view's **document**: every row kind the native
pane lists — file headers with their rule, fold chevron, status glyph and reviewed
mark, hunk headers with their `@@` ranges and reviewed mark, the unified diff lines
with their line-number gutter, syntax-coloured body and insertion/deletion row
tints, comments with their classification badges, the review summary's header and
its comments, informational rows — with the cursor's row drawn in the pane's
selection appearance.

#### Scenario: The core is reproduced completely

- **WHEN** the native renderer and the plugin are given the same review
- **THEN** the two paint the same row for every row kind the pane lists, across
  additions, deletions, context, the cursor's row, an empty body, each colour the
  highlighter assigns, a folded file, a reviewed file, a reviewed hunk, each file
  status, and each comment classification

#### Scenario: The remainder is a list, not a gap

- **WHEN** the port's proposal is read
- **THEN** every unported behaviour of the pane is named with the reason it is
  unported, classified as document or behaviour, and the readiness document carries
  the same list

#### Scenario: The document half is closed when it is closable

- **WHEN** a remainder entry is classified document
- **THEN** it is closed by publishing the facts the row is drawn from, rather than
  left on the list

### Requirement: A reproduction whose native pane is not refactored is validated by frame equality

When a port does **not** refactor the native pane to draw the view tree it is
compared against, the tree builder SHALL be pinned to the **untouched** native
renderer by painting both and requiring the resulting frames to be identical.

Comparing a plugin only against a tree builder written in the same change is
insufficient: two functions agreeing about a format neither is obliged to match is
not evidence. The frame comparison is what closes the chain onto what the pane
paints today.

The reason the native pane was not refactored MUST be recorded, and it MUST be a
property of the pane rather than a preference — for this pane, that its painter
windows a body by character count against a resolved width, which no geometry-free
tree can express.

#### Scenario: The tree builder is pinned to the renderer

- **WHEN** the geometry-free tree and the native renderer's row are each painted at
  the same width
- **THEN** the two buffers are identical cell for cell

#### Scenario: The native pane is unchanged

- **WHEN** the port's diff is inspected
- **THEN** the native paint path is untouched and no pinned frame moves

### Requirement: The view tree's node budget is a whole-tree bound a per-row pane cannot respect

The migration record SHALL state that the view tree's node budget is a bound on a
whole converted tree, while a pane's cost is per row — so a pane whose rows have
unbounded internal structure cannot keep inside the budget by publishing fewer
rows alone.

The measurement MUST be recorded with the pane that produced it: how many nodes a
representative row of this pane costs, how many rows the budget therefore permits,
and that a pathological row can exceed the budget at any row cap.

The consequence MUST be recorded too: the budget is spent building rows the kernel
then windows away, because the plugin builds every row it publishes and the kernel
chooses the visible slice afterwards.

#### Scenario: The budget refuses a real diff

- **WHEN** a plugin returns one row per line of a diff of a few hundred lines with
  syntax-coloured bodies
- **THEN** conversion refuses the tree for exceeding the node budget, and the pane
  reports that rather than drawing a shorter diff

#### Scenario: The bound is recorded, not silently absorbed

- **WHEN** the readiness document is read
- **THEN** it names the node cost of a row, the row cap that cost forces, and the
  two ways out with the reason neither is designed yet

### Requirement: The session list is reproduced by a bundled plugin

The session list SHALL be reproduced by a bundled plugin, and that plugin's view
tree MUST equal the tree the native pane builds from the same rows.

This pane is the architecture's gate: the design says every user-visible surface
is a plugin *including the session list*, so a session list that could only be
kernel-drawn would make the plugin surface second-class by demonstration. The
reproduction MUST therefore cover the parts that make the pane hard rather than
the parts that are easy — the repo-group headers, the animated status glyph, the
nesting prefix for a child session and for a child whose parent renders elsewhere,
the remote and worktree marks, the search emphasis on matched characters, the
agent's reported text, and the cursor's row drawn across the pane's whole width.

Equality MUST be asserted across several content variants, and the comparison MUST
be run at a pane size at which the kernel's geometry step adjusts nothing — with
that size's sufficiency asserted rather than assumed.

The plugin MUST declare exactly the capabilities it uses, so the result is
evidence about what a third party could build rather than about a privileged path.

#### Scenario: The plugin's tree equals the native pane's

- **WHEN** the bundled session-list plugin renders against a published session list
- **THEN** its view tree equals the tree the native pane builds from the same rows

#### Scenario: The animated row is not exempted

- **WHEN** a row's session is working, so its status glyph animates
- **THEN** the equality covers that row's motion node — its frames, its rate and
  its key — rather than comparing everything except the animation

#### Scenario: The plugin claims no more than it uses

- **WHEN** the plugin's manifest is read
- **THEN** it declares rendering and the sessions capability and nothing else

### Requirement: A spike's recorded conditions are re-checked by the port that depends on them

Where a design spike recorded a conditional verdict, the change that implements
what it measured SHALL re-check each condition and state whether it still holds.

A condition that has since been satisfied MUST name what satisfied it. A condition
that is **not** satisfied MUST NOT be worked around inside the bundled plugin: it
MUST be recorded as unmet, with its measurement and the cost of closing it, so the
next pane is not built on a verdict that has quietly expired.

#### Scenario: A condition has been satisfied

- **WHEN** the port finds that a condition the spike set is now met
- **THEN** the change records which host capability met it

#### Scenario: A condition is unmet

- **WHEN** the port finds that a condition the spike set is still unmet
- **THEN** the audit records it as open, with the measured consequence, rather
  than the plugin compensating for it in a way a third party could not

### Requirement: A plugin's view of kernel state trails the kernel's by the render interval

The host SHALL be honest about the latency between a change in kernel state and a
plugin pane redrawn from it, and SHALL keep that latency bounded by a **rate
ceiling** rather than by a fixed render cadence. A pane is rendered when a source it
reads moves; a change arriving after the ceiling's interval has elapsed is rendered
immediately, and one arriving inside it waits out the remainder. So the trailing
interval is at most that ceiling, and is usually zero.

Because the ceiling is not zero, a pane's **cursor** MUST remain kernel state. A
pane that owned its own cursor would put the ceiling's remainder between a keypress
and the highlight moving, which is a latency a user feels directly, whereas a
published cursor moves in the frame the key was handled and only the plugin's
redrawn copy trails.

The ceiling, its worst case, and the rate at which the state behind it actually
moves MUST be recorded with the change that sets them, and MUST NOT be hidden by
having the plugin drive its own repaint. A port that depends on a plugin pane
reflecting kernel state promptly MUST state which of the two it relies on: that the
trigger is the change, or that the ceiling is small.

#### Scenario: The cursor moves

- **WHEN** the user moves the session-list cursor
- **THEN** the native pane's highlight moves on the next frame, and the plugin's
  reproduction of it is re-rendered because the session source moved — immediately
  unless a render pass happened within the ceiling's interval

#### Scenario: The interval is recorded

- **WHEN** a port depends on a plugin pane reflecting kernel state promptly
- **THEN** the audit records the rate ceiling, its worst-case delay, why the
  user-visible cursor is unaffected by it, and what closing the remainder would cost

#### Scenario: Nothing the pane reads has moved

- **WHEN** kernel state a pane does not read changes, or nothing changes at all
- **THEN** the pane is not re-rendered, so an idle interface enters no plugin VM

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

### Requirement: The unportability of a pane's keys is re-derived from the source

The record of which host powers a pane's key surface lacks SHALL be re-derived
from the source tree by a test, one probe per power, so that closing one fails
the record rather than leaving it to expire unnoticed. A probe MUST be scoped to
the declaration it reads, and a failure MUST name the power whose verdict
changed.

The test MUST distinguish a power the pane model withholds **by design** from
one the vocabulary merely has not spelled, so that closing every cheap row
cannot be mistaken for portability.

It MUST NOT be merged into the teardown inventory, which answers whether the
native renderer may be deleted — an answer that is already no for an unrelated
reason and would be unchanged either way.

#### Scenario: A view write is added later

- **WHEN** a host change gives a plugin any way to move a cursor or take focus
- **THEN** the test fails and names that blocker, so the pane's verdict is
  revisited in the change that closed it

#### Scenario: A record write is not mistaken for a view write

- **WHEN** the host gains a further binding that changes a record
- **THEN** the verdict is unchanged and the test still reports the view-write
  blocker as open

#### Scenario: The verdict still holds

- **WHEN** nothing relevant has changed
- **THEN** the test passes, and it also asserts that the bundled plugin declares
  no input capability and no keybinding

### Requirement: A ported pane's scroll track is drawn by the kernel from a declared list

Where a native pane reserves a column of its own area for a scroll track, the
port SHALL move that reservation into the pane's tree — the list declares that it
scrolls and the kernel reserves the column — rather than leaving the plugin's
copy without one.

The native pane MUST keep resolving the same reservation as numbers for the
things only it can answer: which rows a click may land on, and the geometry the
application records as a drag target. That second resolution MUST use the same
helper the drawing uses, so the column a user can drag cannot drift from the
column that was drawn.

The reproduction's claim MUST then rise to frame equality **including the
track's column** at a size where the pane overflows, and that assertion MUST be
shown not to hold vacuously — a comparison of two panes that both drew no track
would pass while proving nothing.

The plugin's track MUST NOT be draggable. It reports a cursor the plugin does not
own, so mapping a drag onto it would be a write into view state the plugin cannot
write, and the record MUST say so rather than leaving the difference to be found.

#### Scenario: The plugin's copy grows the native pane's track

- **WHEN** the file tree is longer than the pane has lines
- **THEN** the plugin's pane and the native pane paint the same frame, thumb
  column included

#### Scenario: The equality is not vacuous

- **WHEN** the same comparison is made against a pane rendered without the
  declaration
- **THEN** the frames differ, so the passing comparison is evidence that a track
  was drawn

#### Scenario: A pane that reserves no track is unaffected

- **WHEN** another ported list pane that never had a scrollbar is rendered
- **THEN** its frames are unchanged and it gains no track

#### Scenario: The plugin's track is an indicator

- **WHEN** a user drags the thumb in a plugin pane's track
- **THEN** nothing scrolls, and the record names the missing view write as the
  reason

### Requirement: A pane whose keys need powers the vocabulary does not define is recorded, not approximated

The rule that a pane's key surface is ported only when a plugin can act on the
row the user is looking at SHALL also cover keys that need powers no capability
names at all, and the record MUST name each power rather than reporting only that
the port failed.

For the file viewer the record MUST state four blockers, each re-derived from the
source:

- **every key writes view state.** The cursor, the expansion set and the search
  all live in kernel-owned view state, and the kernel-state channel is read-only
  by construction. Unlike the previous pane, *none* of this pane's keys is
  expressible as a record write, so there is no partial key surface to argue
  about.
- **expanding a directory reads the filesystem.** The vocabulary defines no
  filesystem capability, the teardown inventory reserves that name for a
  different v1 power, and granting one here would advance a teardown verdict as a
  side effect of drawing a tree.
- **opening a file launches a process.** The widest capability the host defines
  makes even running an automation a request the kernel fulfils, so spawning an
  editor is outside the model rather than merely unimplemented.
- **the search sub-mode's keys are not rebindable.** While it is active the key
  context falls back to the global one so that every character types into the
  query, which means a ported sub-mode could not satisfy the requirement that a
  ported pane's keys appear in the keybinding editor. A plugin could collect the
  same keystrokes and they would search nothing, because the search's effect —
  revealing matches by expanding directories, moving the cursor between them, and
  marking which rows matched — is kernel state with no channel inward.

The record MUST also state what a handover of this pane would delete, which is
not only a renderer: this pane's **model** lives in the module the teardown
removes, and so does the windowing rule every plugin list is drawn by. Lifting
either MUST NOT be done as preparation while the handover remains blocked for
independent reasons.

#### Scenario: The port ships the rendering and not the keys

- **WHEN** the file viewer's plugin is inspected after the port
- **THEN** it declares no input capability and no keybinding, and the native
  pane's keys still do what they did

#### Scenario: A missing power is named rather than approximated

- **WHEN** the record is read for the key that expands a directory
- **THEN** it names the filesystem read and the reason the capability stays
  undeclared, rather than reporting the key as merely unimplemented

#### Scenario: The sub-mode's fixed keys are recorded against the parity bar

- **WHEN** the record is read for the search sub-mode
- **THEN** it states that its keys are fixed rather than rebindable and that a
  plugin-collected query would filter nothing

#### Scenario: The pane's model is not lifted as preparation

- **WHEN** the handover remains blocked for the build and the view write
- **THEN** the pane's state machine stays where it is, and the record says why
  moving it would be motion without a destination

### Requirement: A port states what its capability still refuses when it needed no widening

When a port was expected to widen a capability and did not, the record SHALL say
so and MUST tabulate what the capability grants against what it still refuses,
with the reason each refusal survives — so that "it sufficed" is a measurement
rather than an omission.

For the file viewer the record MUST state that the missing parity is powers
rather than facts: a path would be needed only in order to act on a file and
acting is a process launch, contents would be needed only to preview a file which
this pane never does, and the one fact the pane draws that the section withholds —
the search query — is drawn only inside a bar the host surface cannot describe.

#### Scenario: The record accounts for a capability that did not grow

- **WHEN** the port's design is read
- **THEN** it lists what the section carries, what it refuses, and why each
  refusal is still correct

#### Scenario: A refusal is justified by reachability, not by taste

- **WHEN** the refusal of a file's path is read
- **THEN** the reason given is that no ported key could use it, because acting on
  a file needs a power the host does not define

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
"a pane provides that list". The flag was a proxy that held only while the kernel drew the
target pane unconditionally; kept, a movement key at the edge would move focus into a
pane that is not on screen.

The reproduction's own declining half MUST be removed by that handover, since on the
keyboard route the plugin is never asked.

Once one of the two panes is handed over, the wrap SHALL be recorded as **not** a blocker
for the other's handover, and that MUST be asserted rather than described — otherwise a
later reader re-derives it as one, since it was a row in the first pane's gate for as long
as that pane held its own keys. The assertion MUST name the two facts that make it a
non-issue: both ends are kernel focuses, and the condition is already the pane's presence.

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

#### Scenario: The second pane's refusal does not list the wrap

- **WHEN** the other pane of the pair has its handover refused
- **THEN** the wrap is not among the reasons, and a rule asserts that both ends are
  kernel focuses and that the condition is the pane's presence

### Requirement: A recorded expectation is derived from the native pane and checked against it

A recorded expectation for a ported pane SHALL be generated from the **native**
builder, not from the plugin, and the change that records it MUST assert that the
native builder still reproduces it. While both sides exist, both edges MUST hold:
the recording equals the native tree, and the plugin equals the native tree.

A recording captured from the plugin — or captured after the native builder is
gone — MUST NOT be treated as an expectation, because it freezes whatever the
plugin does as correct, including a defect, and the resulting test can never fail
for the reason it exists.

Recording MUST therefore happen in a change that does not also delete the native
builder, so that the recording's provenance is demonstrable in the run that
introduces it.

**Every pane a bundled plugin reproduces SHALL carry a recording**, whether or not
its handover has been attempted. An earlier form of this requirement was owed by
an *attempt*: a pane whose handover was attempted had to be recorded before the
attempt concluded, either way. That trigger is too late and too weak. It is too
late because three panes' attempts concluded before the rule existed and inherited
the differential oracle it forbids; it is too weak because an attempt is a human
decision to begin work, so a pane nobody attempts keeps its differential oracle
indefinitely — and the change that eventually deletes the native builder is the
one least able to notice that the evidence went with it. Reproduction is the
earliest moment at which the recording is both **owed** and **provable**: the
plugin exists, so there is something to constrain, and the native builder exists,
so the baseline can be shown to be the pane's.

The requirement SHALL be enforced by the tree rather than by convention: a pane
whose oracle holds no recording MUST NOT be recorded handed over, so the native
builder stays protected until the recording exists. See
`migration/teardown`'s handover conditions.

A pane recorded structurally unportable, with no bundled plugin reproducing it, is
owed no recording — there is no plugin to constrain and no handover to gate.

#### Scenario: The recording is captured while both sides exist

- **WHEN** a recorded expectation is introduced for a ported pane
- **THEN** the native builder is still present, the recording equals its tree for
  every case, and the plugin equals its tree for every case

#### Scenario: A recording is proposed from the plugin's output

- **WHEN** a recorded expectation would be generated from the plugin rather than
  from the native builder
- **THEN** it is refused, because a plugin defect would become the expectation

#### Scenario: The plugin diverges after the recording exists

- **WHEN** the plugin's tree changes in any recorded respect
- **THEN** the recorded comparison fails and names the node that moved

#### Scenario: A pane is reproduced and never attempted

- **WHEN** a bundled plugin reproduces a native pane and no handover of it is
  attempted
- **THEN** the pane still carries a recording checked against its native builder,
  because reproduction is what makes the recording owed

#### Scenario: A handover attempt is refused

- **WHEN** an attempt to hand a pane over concludes that the native pane stays
- **THEN** the pane's recorded expectation exists and is checked against its
  native builder, so the next attempt does not start from a differential oracle
  again

#### Scenario: A pane's oracle is still differential when its handover is proposed

- **WHEN** a handover would delete a native builder that is the only right-hand
  side its pane's oracle has
- **THEN** the handover is refused by the teardown inventory, naming the missing
  recording, rather than being permitted and repaired by dropping the assertion

#### Scenario: A surface is recorded unportable

- **WHEN** a native surface is recorded as structurally unportable and no bundled
  plugin reproduces it
- **THEN** no recording is owed for it

### Requirement: A compact recorded expectation is exhaustive over the view tree

A recorded expectation MAY be a compact rendering rather than a full structural
dump, but its renderer SHALL destructure every view-tree variant and every style
field by name, with no rest pattern and no wildcard arm. Adding a field to the
view tree MUST fail to compile until the recording accounts for it.

A compact form is required to stay legible, because an expectation no reviewer can
read is one every update rubber-stamps — and a rubber-stamped expectation records
what the code last did rather than what the pane should show. Exhaustiveness is
what stops legibility from being bought with an omission: a fact absent from the
recording is a fact the oracle no longer constrains.

#### Scenario: A view-tree field is added

- **WHEN** a new field is added to a view-tree node or to a text style
- **THEN** the recording's renderer fails to compile until it prints or
  deliberately accounts for that field

#### Scenario: A style fact is set

- **WHEN** a node carries any non-default style fact
- **THEN** the recording shows that fact

#### Scenario: The recording stays reviewable

- **WHEN** a fully populated pane is recorded
- **THEN** the recording is a line-per-node rendering a reviewer can read, not a
  structural dump of every default-valued field

### Requirement: One recorder serves every recorded pane

The renderer that produces a recorded expectation SHALL be single-sourced across
the panes that record one, rather than copied per pane.

The renderer's exhaustiveness over the view tree is the property that stops a
compact recording from silently omitting a fact, and that property is worth as
much as the number of copies of it: N copies are N formats that can drift, and a
field added to the view tree would have to be accounted for N times to keep the
oracle whole. Single-sourcing also makes two panes' recordings comparable, since
a difference between them is then a difference between the panes.

A test file MAY hold private helpers that read the source tree, which are
duplicated deliberately elsewhere in the suite; the constraint here is specific to
the renderer that defines what a recording *contains*.

#### Scenario: A second pane records its tree

- **WHEN** another ported pane gains a recorded expectation
- **THEN** it uses the existing recorder rather than its own copy

#### Scenario: A view-tree field is added while several panes record

- **WHEN** a field is added to a view-tree node or to a text style
- **THEN** exactly one place fails to compile, and fixing it restores every
  pane's recording at once

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

### Requirement: A geometry-free row clips where the native row ellipsizes

A view tree carries no width. While the catalogue had no way to say otherwise, a pane
whose native renderer **truncates** a row to the pane's width was recorded as
diverging in that row's last columns, attributed to the same missing fact that blocks
the pane's width-dependent layouts.

Once the catalogue **can** say it — a run declaring that it yields its width, which
the kernel ellipsizes — a divergence is no longer an acceptable record for a pane that
only needed a fit. Such a pane SHALL close it, and closing it has two halves that MUST
land together:

- the reproduction declares the yielding run, and
- the **native** pane stops fitting the text itself.

Both, because a pane that keeps cutting the string in its own tree while its
reproduction declares the fit produces trees that differ **by construction** — the
native tree carries an already-cut name and the plugin's carries a whole one — so no
width makes the two equal and the equality test can only be kept passing by comparing
at a width where the fit is a no-op.

A pane MUST NOT close it by publishing a resolved width, and MUST NOT close it by
publishing an already-fitted string. A width is resolved during a frame while the
snapshot is published on the tick, and the two panes' rects are not the same rect
while both exist; a fitted string would make the snapshot carry the pane's rendering
rather than the model's fact.

Where the fitted text is split into several runs — a name segmented at a search's
matched offsets — the runs MUST be cut as one piece of text, so the pane and its
reproduction agree with the kernel's own fitting rather than each cutting per run.

A pane whose divergence is still recorded rather than closed MUST say which of the two
halves is missing, so "the vocabulary cannot express it" is never inferred from a
record that means "this pane has not adopted it".

#### Scenario: A row that fits is identical

- **WHEN** a truncating row kind is painted at a width its text fits in
- **THEN** the plugin's frame and the native frame are identical cell for cell

#### Scenario: A row that overflows is identical too

- **WHEN** the same row is painted at a width its text overflows, after both halves
  have landed
- **THEN** both frames end that run in an ellipsis at the same column, and every run
  after it is still drawn

#### Scenario: A row that overflows clips

- **WHEN** a pane has not adopted the declaration and one of its rows overflows
- **THEN** the plugin's row is clipped at the pane's edge, the native row ends in an
  ellipsis one column earlier, and the record names the **native pane's fitting** as
  what is outstanding rather than the vocabulary

### Requirement: A style token is added when the palette field it names has no token

When a ported pane draws in a palette field the token vocabulary does not name, the
port SHALL add a token for that field rather than reuse a token resolving a
different field.

A near-miss token is worse than a missing one: it paints a plausible colour, so the
equality test passes on the default theme and the pane diverges only on a custom
theme that sets the two fields apart — which is the case the token vocabulary exists
to serve.

#### Scenario: A near-miss token is refused

- **WHEN** a pane needs the palette's diff colours for a file header's insertion and
  deletion counts, and the vocabulary's insertion token resolves a different field
- **THEN** tokens for the diff colours are added, and the near-miss token is left
  resolving what it already resolved

### Requirement: A surface occupying more than one seat is not handed over as one pane

When a native surface occupies **two or more** of the interface's seats at once, the
migration record SHALL treat it as needing every one of them before it may be handed
to a plugin, and SHALL name which seats and what each contributes.

A pane replacement is per pane, and a plugin declares its panes in its own manifest,
so a surface spanning two columns is not "a pane with an extra region": it is two
panes that must be granted, focused and navigated together. Handing over only the
seat a plugin can take would remove the other from the interface while the
replacement still looked complete.

The code-review view is the first such surface: its diff owns the central pane, and
its changed-files list owns the file-viewer column with a focus and a keyboard of its
own, force-shown for as long as a review is open.

#### Scenario: A two-seat surface is refused as one pane

- **WHEN** a handover of a surface occupying two seats is attempted
- **THEN** it is refused, and the record names both seats and what handing over only
  one would remove

#### Scenario: The second seat is a pane in its own right

- **WHEN** the record describes the second seat
- **THEN** it states that the seat has its own focus and its own keys, so it is a
  second pane to be granted rather than a region of the first

### Requirement: A pane whose keys are a capture rather than actions loses them silently

When a native pane's keyboard is implemented as a **capture keyed on the focused
surface** rather than as bindable actions, the migration record SHALL state that a
handover loses those keys with nothing to name in their place.

The three earlier refusals could each point at the scoped actions a plugin binding
would have to replace, because those panes' keys are `Action`s in a `KeyContext`. A
capture has no such surface: the keys are not in the keybinding vocabulary, so they
are not rebindable, the interactive editor has never listed them, and a configuration
file cannot restore them after a handover.

The consequence MUST be recorded as an ordering constraint rather than only as a
blocker: such a pane's keys have to *become* actions before a handover can be
described, and that is a change to the keybinding vocabulary rather than to the
plugin surface.

#### Scenario: A capture-keyed keyboard is named as such

- **WHEN** the record examines the pane's keys
- **THEN** it states that the handler runs ahead of the keybinding lookup on the
  focused surface, that the keybinding contexts name no such surface, and that a
  focused plugin pane therefore resolves none of the pane's keys

#### Scenario: The ordering is stated

- **WHEN** the work the handover would need is ordered
- **THEN** turning the pane's keys into scoped actions precedes any plugin-facing
  change, because until then there is nothing for a plugin binding to claim

### Requirement: A mouse channel carrying a row cannot express a click that means a column

The migration record SHALL state where a pane's mouse surface exceeds the row channel
a plugin pane receives, entry by entry, and SHALL distinguish a *missing target kind*
from a *missing coordinate*.

A plugin pane's click reports the row of the pane's outermost list. A pane whose
mouse surface includes buttons, a draggable scrollbar, wheel scrolling or picker
entries is missing target **kinds**, each of which a wider event could carry. A pane
where the **column** a click landed in changes the meaning of the click is missing a
coordinate the row channel cannot hold at all — and that is the stronger statement,
because no additional target kind closes it.

The code-review view has both: eleven footer buttons, a scrollbar, a wheel and a
target picker, plus a paired side-by-side row where the half clicked decides which
side of the diff a comment attaches to.

#### Scenario: The two kinds of gap are separated

- **WHEN** the record enumerates the pane's mouse surface
- **THEN** the targets a wider event would carry are listed apart from the coordinate
  the row channel cannot carry, with the reason the second is not a subset of the
  first

### Requirement: A refused row states whether it is narrower than a row already refused

When a handover is refused for a reason another pane was already refused for, the
record SHALL state whether **this** pane's version of that reason is narrower, and if
it is, that it is the cheaper place to start.

A gate that only repeated "the cursor is kernel state" would hide a real difference:
the session list's cursor *is* the application's active session, so making it
writable is the widest grant in the host, while the code review's cursor is a row
inside a view the user already opened. Two rows spelled alike with very different
prices should not read alike.

#### Scenario: The narrower row is identified

- **WHEN** two panes are refused for a cursor the kernel owns
- **THEN** the record states which pane's cursor is the narrower grant, and names it
  as the order the work would be done in

### Requirement: A refusal adds no capability that has no consumer

A change that records a refusal SHALL NOT add a capability, a binding or a manifest
field in the same change.

A capability added alongside a verdict has no pane using it — the defect the earlier
gates identified in `input`, `tasks-write` and `automations-write`, which existed
before any bundled plugin declared them. The refusal's job is to state what is
needed and in what order; granting it belongs to the change that consumes it.

#### Scenario: The refusal changes no source

- **WHEN** the change recording the refusal is inspected
- **THEN** it adds tests and documentation and no capability, binding or manifest
  field

### Requirement: A divergence a port enumerates becomes a gate row when its pane is refused

A port MAY enumerate a divergence from the native pane in its own test file, asserting
the inequality so the divergence cannot close unnoticed. That is a **measurement** and
it SHALL be kept.

When that pane's handover is refused, every enumerated divergence SHALL **also** appear
as a row in the pane's handover gate, with its own probe re-derived from the source. A
divergence recorded only in a test's documentation is a verdict written in prose, and a
verdict written in prose is a fact about a build that expires without telling anyone —
the same rule that put the refusal in a gate rather than in a document, applied to a
`///` block.

The two MUST NOT be collapsed into one. They fail for different reasons: the port's
assertion fails when the divergence **closes**, forcing the port to be revisited; the
gate row fails when the *tree* stops matching the recorded verdict. Deleting either
leaves the other unable to fail for its own reason.

A gate that already carries some of a pane's enumerated divergences and not others MUST
be completed rather than left inconsistent, since the omission is otherwise read as a
judgement that the missing ones do not block.

#### Scenario: A refusal is recorded for a pane with enumerated divergences

- **WHEN** a pane's handover is refused and its port enumerates divergences
- **THEN** each divergence has a row in the pane's gate, tagged by kind, with a probe
  that re-derives it

#### Scenario: A divergence closes

- **WHEN** the pane and its reproduction stop diverging
- **THEN** the port's inequality assertion fails and the gate row's recorded verdict
  disagrees with the tree, so both ask to be revisited

### Requirement: A pane whose window is its widget's is refused until the window is a seam

A native pane MAY resolve which of its rows are on screen through a list widget that
keeps its own scroll offset. Where **other behaviours are derived from that offset** —
clipped-row indicators, click hitboxes, the position of a row the kernel inserts — the
pane SHALL NOT be handed over until that window is a seam both occupants resolve
through.

The reason is not that the reproduction cannot scroll. It scrolls by the kernel's own
rule, from a declared cursor; the rule is simply a **different** one, over a different
row count where the native widget folds one row into another. So at any height where the
list overflows the two panes show different rows — which for a pane whose selection
drives what other panes display is a behavioural change, not a rendering divergence.

The refusal MUST enumerate each behaviour derived from the offset separately, because a
reader who sees only "which rows are on screen" concludes the gap is a wiring detail.

The window MUST NOT be closed by redefining the kernel's own windowing rule to match one
pane's widget: that rule is what every plugin list and several native panes scroll by, so
a change for one pane changes all of them.

The row SHALL be closed instead by converging the **pane** onto that rule — the native
pane resolving its window, its clipped-row indicators and its click hitboxes through the
same painter and the same windowing helper its reproduction goes through, and both trees
folding a header and the row it heads into one list item so one index names the same row
in both. The direction is the constraint: the shared rule is unchanged, and it is the
pane scheduled for deletion that moves.

The relocation of anything the widget's window feeds MUST be ordered **after** this
decision, since what a windowing seam looks like decides where those functions live.

#### Scenario: The two windows disagree

- **WHEN** a list longer than the pane is drawn by the native pane through a list widget
  and by its reproduction through the kernel's rule
- **THEN** both keep the cursor visible, the sets of other visible rows differ, and the
  gate records the difference as structural

#### Scenario: Redefining the shared rule is refused

- **WHEN** matching the widget's behaviour by changing the kernel's windowing helper is
  proposed
- **THEN** it is refused, because that helper is shared by every plugin list and several
  native panes

#### Scenario: The pane converges onto the shared rule

- **WHEN** the native pane is changed to window through the kernel's helper, to fold a
  header into the row it heads, and to read its indicators and hitboxes off the paint
- **THEN** the two panes draw the same rows at the same height, and the row is recorded
  closed with the behaviour it changed

### Requirement: The file viewer is drawn by its bundled plugin, and its renderer is deleted

The file viewer SHALL be handed over to its bundled plugin: `src/ui/file_viewer.rs` is
deleted, and the pane a user opens is the plugin's, drawn from the seat the native pane
occupied, gated by the flag that always gated it and bound to the key that always showed
it.

The handover SHALL grant the plugin **nothing**. The pane's seven scoped actions —
including the two whose effects are a **directory read** and a **process launch** —
remain the kernel's, resolved because the pane declares that it is thurbox's file viewer
and performed against the kernel's own state. The `files` capability MUST still publish a
basename per row and nothing else: no path, no contents, no directory listing, and no
search query.

The pane's search bar SHALL remain the kernel's, drawn as seat chrome in the rows it
always occupied.

The tree the pane shows MUST go on being rebuilt when the active session changes, and
MUST NOT be rebuilt while the pane is off screen — the native pane read no directory with
its column closed, and neither may the seat.

#### Scenario: The pane a user opens is the plugin's

- **WHEN** the key that shows the file viewer is pressed
- **THEN** the column appears in the position it always had, showing the plugin's tree,
  focused, and the key that hid it hides it

#### Scenario: The keys still do what they did

- **WHEN** the pane holds focus and its scoped keys are pressed
- **THEN** the cursor moves, a directory expands and collapses, a file opens in the
  configured editor, and the search runs — all performed by the kernel, all still
  rebindable

#### Scenario: No capability was widened to close the handover

- **WHEN** the plugin's manifest and the host's bindings are inspected after the handover
- **THEN** the manifest holds `render` and `files`, no binding reads a directory or a
  file, no binding launches a process, and the published row carries no path

#### Scenario: A hidden pane reads no directory

- **WHEN** the pane is not on screen and the active session changes
- **THEN** no directory is read for it

### Requirement: A row another change would flip is re-verdicted by that change

Where a change makes a **different** pane's recorded gate row probe differently, that
change SHALL re-verdict the row in the same commit: rewriting what the row stands on, and
its probe, to the fact that now decides it.

A row MUST NOT be left to flip on its own. A gate row's probe is a proxy for its reason,
and a proxy that stops matching its reason reports the opposite of the truth — "the seat
now exists, so the pane is unblocked" where the seat existing is precisely what makes the
pane's problem sharper.

Where the re-verdict makes the row's reason stronger rather than weaker, the change SHALL
say so, so that a reader of the two commits can see the verdict moved for a reason rather
than to keep a suite green.

#### Scenario: A seat another pane's gate asserts is absent is added

- **WHEN** a handover names a seat that another pane's gate row probed for the absence of
- **THEN** that row is rewritten in the same change to stand on what now blocks it, and
  the pane's verdict is unchanged unless its reason genuinely went

#### Scenario: The re-verdict is a strengthening

- **WHEN** the new fact makes the other pane's handover harder rather than easier
- **THEN** the change records that, rather than presenting the rewrite as bookkeeping

### Requirement: The session list's ordering model is owned by the kernel, not by its pane

The order thurbox navigates its sessions in SHALL live in the pure-data layer, not in the
module that draws the session list. This covers the comparator the session-list pane and
`Ctrl+J`/`Ctrl+K` navigation share, the nesting of a child under its parent within a repo
group, the repo grouping keys and labels, the reorder primitive behind
`Shift+J`/`Shift+K`, the alphabetical sort behind `Shift+S`, the fuzzy match positions
global search resolves per session, and the width-free resolution of a rendered row.

The requirement is independent of any handover. A comparator that keyboard navigation
depends on is model, and a rendering module that also holds it makes the coordinator
depend on the layer that is supposed to depend on *it*.

The relocation SHALL NOT change behaviour: no relocated function's body is edited, no
signature is widened, and every caller is updated to name the model where the model now
lives.

Geometry SHALL stay behind. The fit of an agent's reported text into the columns a row has
left is a function of a resolved pane width, so it remains with the pane; the relocated
row type carries that field unset, and the pane fills it.

#### Scenario: Navigation names its own model

- **WHEN** the coordinator orders sessions for `Ctrl+J`, reorders them for `Shift+J`, or
  sorts them for `Shift+S`
- **THEN** it calls the pure-data layer, and no call from the coordinator into the
  rendering layer remains for any of the three

#### Scenario: The pane keeps its geometry

- **WHEN** the pane resolves how much of an agent's text fits after a session's name
- **THEN** that fit stays in the pane's module, and the relocated resolution leaves the
  fitted field unset

#### Scenario: The relocation is behaviour-preserving

- **WHEN** the model has moved
- **THEN** the pane draws the same frame, the recorded pane oracle is unchanged, and no
  snapshot moves

### Requirement: A refused pane's remaining structural blocker is stated with a measurement

Where a pane's handover is refused and one structural row remains, the change that closes
the others SHALL record **why the remaining row is not closable by it**, as a measurement
of the two behaviours that disagree rather than as a restatement of the row.

A row asserting that two rules "differ" is checkable only in the sense that both rules
exist. What decides whether a handover would regress is the size and shape of the
disagreement, and that is a fact a reader cannot re-derive from prose.

Where the row names more than one behaviour, the measurement MUST cover the ones a policy
choice would not settle, so that "pick the other scroll rule" is visibly not an answer.

#### Scenario: The remaining row is measured

- **WHEN** the last structural row is left open
- **THEN** the change records the two windows' actual output over a list longer than the
  pane, and the reader can see which rows each would show

#### Scenario: The measurement covers what policy cannot settle

- **WHEN** the row also names row granularity and the index space a click resolves through
- **THEN** those are stated as separate findings, because neither is decided by which
  scroll rule is chosen

### Requirement: A divergence closed by giving the plugin a predicate keeps its measurement

Where a reproduction diverges from its native pane because the plugin's language cannot
express a rule the kernel applies, the divergence SHALL be closed by giving the plugin
**the predicate**, not the answer — and the port's enumerated case SHALL be inverted
rather than deleted.

Giving the answer is the tempting closure and it is refused: publishing the resolved
text moves a presentation decision into the publication and leaves the next pane wanting
the same rule exactly where this one was.

The inverted case MUST keep a guard that its fixture still exercises the difference. A
case that once asserted an inequality and now asserts an equality can pass by having
stopped testing anything, so it MUST also assert that the input still differs from its
resolved form.

#### Scenario: The predicate closes the divergence

- **WHEN** a reproduction cannot apply a kernel rule because its language has no
  equivalent
- **THEN** the host exposes the rule as a pure function, and the pane goes on deciding
  where to apply it

#### Scenario: The enumerated case inverts

- **WHEN** such a divergence closes
- **THEN** the port's inequality assertion becomes an equality assertion under a name
  saying so, and the gate row is re-verdicted with its probe re-derived

#### Scenario: The inverted case still exercises its fixture

- **WHEN** an inverted case runs
- **THEN** it asserts that the fixture's input still differs from its resolved form, so
  an equality cannot pass by testing nothing

### Requirement: The code review's changed-files list windows by the kernel's rule

The changed-files list SHALL resolve its visible rows, its click hitboxes and its
selection appearance through the kernel's shared painter and the kernel's shared
windowing rule, and SHALL NOT compute a window of its own.

The pane's folder-tree grouping — which directories head which files, at which depth, and
which file each row names — SHALL live in the layer that owns the diff's data rather than
in the pane that draws it, so a second drawer of this list resolves the same rows rather
than reimplementing the sort. The rows MUST name each file by its index into the review's
own file list, so a row survives crossing to a reader that holds no diff.

The list's selection appearance SHALL be carried by the row's own runs rather than decided
while painting the window, since the row is built by whoever draws the list and the window
is resolved by the kernel.

The pane's pre-existing row rendering SHALL be retained as a test oracle and asserted
buffer-equal — glyph, colour and modifier — at a width that fits and a width that truncates
a directory name, so the convergence is evidenced rather than claimed.

This change SHALL hand no pane over: every row of the code review's handover gate keeps
its verdict, and the row that names this list stays blocked while two kernel surfaces
contest its column.

#### Scenario: The list scrolls by the shared rule

- **WHEN** the changed-files list holds more files than its column has rows and the diff's
  cursor is deep in the list
- **THEN** the window opens the kernel's margin above the current file and clamps at the
  list's tail, rather than pinning that file to the last visible row

#### Scenario: The unscrolled frame does not move

- **WHEN** the list fits in its column
- **THEN** the painted buffer equals what the pane's spans painted, cell for cell,
  including a directory name cut short by a narrow column

#### Scenario: A directory header is not clickable

- **WHEN** the painter reports one row per list child and a directory header is one of them
- **THEN** that row yields no hitbox and the file rows keep the indices they had, so a
  click still jumps the diff to the file it names

#### Scenario: The gate is unmoved

- **WHEN** the convergence lands
- **THEN** the code review's handover gate records the same five outstanding rows, and the
  change states that it hands nothing over

