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

#### Scenario: The default interface is unchanged

- **WHEN** thurbox starts with the bundled plugin present and no stored
  visibility choice
- **THEN** the plugin's pane is off screen and the native pane renders as before

#### Scenario: The native renderer is still protected

- **WHEN** the teardown inventory is checked after the port
- **THEN** the native pane's renderer is still required to exist, because the
  pane has not been handed over

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

