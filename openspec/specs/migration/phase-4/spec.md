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

#### Scenario: The default interface is unchanged

- **WHEN** thurbox starts with the bundled plugin present and no stored
  visibility choice
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

#### Scenario: Rows fit in the column

- **WHEN** every row fits the width and the list fits the height
- **THEN** the native pane's tree and the plugin's are equal

#### Scenario: A row is wider than the column

- **WHEN** a row's label exceeds the native pane's width
- **THEN** the native pane fits it and the plugin's copy does not, and a test
  asserts that difference and names the node that would remove it

#### Scenario: The list is longer than the pane

- **WHEN** there are more rows than the pane has lines
- **THEN** the native pane windows them around the selection while the plugin's
  copy draws from the first row, and a test asserts that difference

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

The chosen core is the code-review view's **unified diff stream's lines**: the
line-number gutter, the syntax-coloured body, the insertion and deletion row
tints, and the cursor's row.

#### Scenario: The core is reproduced completely

- **WHEN** the native renderer and the plugin are given the same diff line
- **THEN** the two paint the same row, across additions, deletions, context, the
  cursor's row, an empty body, and each colour the highlighter assigns

#### Scenario: The remainder is a list, not a gap

- **WHEN** the port's proposal is read
- **THEN** every unported behaviour of the pane is named with the reason it is
  unported, and the readiness document carries the same list

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
plugin pane redrawn from it: a plugin renders on the render worker's own cycle, so
its copy of any published state — including which row a cursor is on — may trail
the kernel's by up to one render interval.

Because of that, a pane's **cursor** MUST remain kernel state. A pane that owned
its own cursor would put that interval between a keypress and the highlight
moving, which is a latency a user feels directly, whereas a published cursor moves
in the frame the key was handled and only the plugin's redrawn copy trails.

The interval and its consequence MUST be recorded with the port that measured
them, and MUST NOT be hidden by having the plugin drive its own repaint.

#### Scenario: The cursor moves

- **WHEN** the user moves the session-list cursor
- **THEN** the native pane's highlight moves on the next frame, and the plugin's
  reproduction of it moves when the worker next renders

#### Scenario: The interval is recorded

- **WHEN** a port depends on a plugin pane reflecting kernel state promptly
- **THEN** the audit records the render interval, why the user-visible cursor is
  unaffected by it, and what closing the gap would cost

