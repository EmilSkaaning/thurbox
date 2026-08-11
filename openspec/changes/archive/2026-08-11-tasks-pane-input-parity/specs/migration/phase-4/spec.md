# phase-4 (delta)

## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: A pane's key surface is ported only when a plugin pane can own the cursor those keys move

A native pane's keys SHALL be reproduced by a plugin only when the plugin can
act on the row the user is looking at. Where it cannot, the port MUST leave the
pane's keys with the kernel and MUST NOT declare bindings that fire against a row
no user can see — the same rule the host already applies when it refuses to
publish a binding it could not deliver.

For the tasks pane the answer is that it cannot, and the record MUST name the
two independent reasons:

- **A cursor is view state and nothing writes it.** Moving the selection,
  scrolling the central pane's preview, focusing the editor and switching to a
  related session are all writes to what the user is looking at, and the
  kernel-state channel is read-only by construction. A capability that changes a
  *record* is not this write, which is a distinction an earlier verdict already
  had to be corrected on.
- **The input path and the cursor path are disjoint.** A plugin receives keys
  only while one of its own panes holds focus, and the published task section
  marks the cursor's row only while the *native* pane holds focus or a search
  preview is moving it. So the two keys that need no new host power — cycling a
  status and deleting a task, both already expressible as record writes — still
  cannot name the row they would act on.

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
