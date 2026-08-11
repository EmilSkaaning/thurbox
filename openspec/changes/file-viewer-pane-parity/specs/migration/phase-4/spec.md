# phase-4 (delta)

## ADDED Requirements

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
