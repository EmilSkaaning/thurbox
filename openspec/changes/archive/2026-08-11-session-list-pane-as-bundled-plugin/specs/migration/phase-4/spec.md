# migration/phase-4 Specification

## ADDED Requirements

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
