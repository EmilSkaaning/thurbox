# plugin-host/kernel-state Specification

## Purpose
Defines how a plugin reads state the kernel owns — sessions, host metrics,
scheduled automations — without reaching into the running application. A plugin
VM runs on its own thread with no clock and no filesystem, so the channel has to
answer three questions at once: what a plugin is allowed to see (a capability per
kind of state), what the kernel must resolve on its behalf (anything a sandbox
cannot compute), and what it must *not* resolve (any string the pane displays,
because presentation is the pane's job). It also has to cost nothing when nobody
is reading, or an installed plugin would tax every idle tick.
## Requirements
### Requirement: Kernel state reaches a plugin as a published snapshot

The host SHALL expose kernel state to a plugin as a snapshot published by
whichever binary owns that state, held in one process-wide slot of pure data,
and read by a binding when the plugin calls it. A binding MUST NOT reach into
the running application, and reading one MUST NOT require any plugin code to run
on the thread that draws.

A publication MUST be atomic: a snapshot replaces the previous one whole, so a
reader observes either the old value or the new one and never a mixture of the
two. Each reader answers from the most recently published snapshot at the moment
it is called.

#### Scenario: Nothing has been published

- **WHEN** a plugin calls a state reader in a process where no snapshot has been
  published
- **THEN** the reader returns nothing, rather than failing or blocking

#### Scenario: A published snapshot is readable

- **WHEN** a snapshot is published and a plugin holding the matching capability
  calls its reader
- **THEN** the reader returns that snapshot's values

#### Scenario: A later publication replaces an earlier one

- **WHEN** a snapshot is published twice with different values
- **THEN** a reader called afterwards sees only the second

#### Scenario: A reader never sees a partial snapshot

- **WHEN** a snapshot is published while a plugin is reading one
- **THEN** the reader observes either the whole previous snapshot or the whole
  new one, never a mixture of their sections

### Requirement: Publishing kernel state costs nothing when nobody reads it

The publisher SHALL build no snapshot unless at least one running plugin holds a
capability that can read one, and MUST NOT publish a snapshot equal to the one
already published. Both properties MUST be observable through counters rather
than asserted in prose, so a regression is a failing test and not a judgement.

Publishing MUST NOT mark the interface as needing a repaint: a plugin pane
repaints when the tree it returns changes, and coupling the two would make an
installed plugin repaint the screen on every state change whether or not its
pane is on screen.

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

### Requirement: The snapshot carries what a plugin cannot derive, and no more

The snapshot SHALL resolve, on the plugin's behalf, exactly those values a
sandboxed plugin has no way to compute: anything requiring a clock, a filesystem
path, a lookup across kernel records, or a rendering decision the kernel owns.
Quantities SHALL be carried as numbers, and the plugin SHALL compose every string
it displays.

Concretely, the snapshot MUST carry time-to-event as an already-resolved
duration rather than an absolute instant, a directory's display name rather than
its path, a referenced record's name rather than only its identifier, and — for
each session status — the glyph and the style token the kernel draws it with.

#### Scenario: A countdown is resolved before publication

- **WHEN** the snapshot describes an event with a known absolute time
- **THEN** it carries the remaining duration, so a plugin with no clock can
  render the countdown

#### Scenario: A path becomes a display name

- **WHEN** the snapshot describes a repository or an additional directory
- **THEN** it carries the name a user sees, not a filesystem path

#### Scenario: A reference is resolved to a name

- **WHEN** the described session has a parent session
- **THEN** the snapshot carries the parent's name, falling back to a shortened
  identifier when the parent is no longer present

#### Scenario: A status carries how the kernel draws it

- **WHEN** the snapshot describes a session's status
- **THEN** it carries that status's label, its glyph, and the style token the
  kernel resolves it to, so two panes cannot disagree about either

#### Scenario: Quantities are not pre-formatted

- **WHEN** the snapshot describes a byte count, a token count, a duration, a
  cost or a percentage
- **THEN** it carries the number, and the plugin formats it

### Requirement: Each kind of kernel state is a separate capability

Reading kernel state SHALL be gated per kind of state, not by one blanket grant.
A plugin granted one kind MUST NOT be able to read another, and the binding for a
kind it was not granted MUST be absent from its environment.

The kinds are the running **sessions**, host resource **metrics**, and scheduled
**automations**.

#### Scenario: A plugin declares one kind

- **WHEN** a plugin declares only the session-reading capability
- **THEN** the session reader is present and the metrics and automation readers
  are absent

#### Scenario: A plugin declares none

- **WHEN** a plugin declares no state-reading capability
- **THEN** none of the three readers is present in its environment

#### Scenario: A plugin declares all three

- **WHEN** a plugin declares all three state-reading capabilities
- **THEN** all three readers are present and each returns its own section

### Requirement: A state reader answers about the active session only when there is one

The session reader SHALL describe the session the user is currently on, and MUST
return nothing when there is none — a fresh thurbox with no sessions is the
normal case, not an error.

#### Scenario: No session exists

- **WHEN** a plugin holding the session capability reads state with no session
  open
- **THEN** the reader returns nothing and the plugin can render a placeholder

#### Scenario: The active session changes

- **WHEN** the user moves to a different session and the snapshot is republished
- **THEN** the reader describes the newly active session

