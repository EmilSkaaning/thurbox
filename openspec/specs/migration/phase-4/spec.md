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

