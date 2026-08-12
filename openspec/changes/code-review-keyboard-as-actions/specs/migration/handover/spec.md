# migration/handover Specification (delta)

## ADDED Requirements

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
