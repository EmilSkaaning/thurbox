# kernel-state (delta)

## MODIFIED Requirements

### Requirement: Publishing kernel state costs nothing when nobody reads it

The publisher SHALL build no snapshot unless at least one running plugin holds a
capability that can read one, and MUST NOT publish a snapshot equal to the one
already published. Both properties MUST be observable through counters rather
than asserted in prose, so a regression is a failing test and not a judgement.

The change gate SHALL be the set of **sources** in which the new snapshot differs
from the published one, and publishing MUST happen exactly when that set is
non-empty. Deriving the gate from the sources rather than from a whole-value
comparison is what lets the publisher tell the render worker *what* moved, so a pane
is re-rendered only for a source it reads. The derivation MUST account for every
field of the snapshot — a field belonging to no source would be a change that never
publishes — and it MUST be equivalent to comparing the two snapshots, which MUST be
checked rather than assumed.

Publishing MUST NOT mark the interface as needing a repaint: a plugin pane
repaints when the tree it returns changes, and coupling the two would make an
installed plugin repaint the screen on every state change whether or not its
pane is on screen. Telling the render worker which sources moved is not a repaint
and MUST NOT become one.

#### Scenario: No plugin can read kernel state

- **WHEN** the publisher runs repeatedly and no running plugin holds a
  state-reading capability
- **THEN** no snapshot is built and the build counter does not advance

#### Scenario: A reader exists and the state is unchanged

- **WHEN** the publisher runs repeatedly while the state it describes does not
  change
- **THEN** a snapshot is built but the publish counter advances at most once

#### Scenario: The state changes

- **WHEN** a value inside the snapshot changes and the publisher runs
- **THEN** the publish counter advances

#### Scenario: Publishing does not repaint

- **WHEN** a snapshot is published while nothing else has changed
- **THEN** the interface is not marked as needing a repaint

#### Scenario: A publication names the sources that moved

- **WHEN** one section of the snapshot changes and the publisher runs
- **THEN** the render worker is told that source moved and is not told about the
  sources that did not

#### Scenario: The source derivation and the equality agree

- **WHEN** two snapshots are compared
- **THEN** the set of changed sources is empty exactly when the two snapshots are
  equal

#### Scenario: A field is added to the snapshot

- **WHEN** a field is added to the published snapshot
- **THEN** the source derivation fails to compile until that field is assigned to a
  source
