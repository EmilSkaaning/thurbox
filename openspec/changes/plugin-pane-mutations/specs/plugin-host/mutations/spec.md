# plugin-host/mutations Specification

## ADDED Requirements

### Requirement: The mutating surface is a closed list of enumerated operations

The host SHALL expose to a plugin only these operations that change kernel
records, and no others:

| Operation | Effect |
|---|---|
| set a task's status | the task takes one of the statuses the kernel defines |
| delete a task | the task is soft-deleted, as the native pane's delete key does |
| set an automation's enabled flag | the automation is enabled or disabled, and its next occurrence is recomputed or cleared to match |
| run an automation | the automation is marked due; the **kernel** fires it |
| delete an automation | the automation and its run history are removed |

Each operation MUST address exactly one existing record by its id and MUST report
whether that record existed. There MUST be no operation that creates a record,
changes a record's text, names a command or a program, executes anything, or
reaches a kind of record other than the two named above.

The list is closed **because** it is the widest reach the host grants: a general
mechanism (arbitrary SQL, a "call any kernel function" binding, or a create-plus-
update pair that can author a command) would make a plugin's reach unreviewable
from its manifest, which is the property the capability model exists to provide.

#### Scenario: A granted operation changes the record

- **WHEN** a plugin holding the task-write capability sets an existing task's
  status
- **THEN** the task's stored status is that status, and the call reports success

#### Scenario: The record does not exist

- **WHEN** a granted operation names an id no record has
- **THEN** the call reports that nothing was changed, and nothing fails

#### Scenario: No operation authors a record

- **WHEN** a plugin's environment is inspected with both write capabilities
  granted
- **THEN** it contains no binding that creates a task or an automation, none that
  changes either one's text, schedule or action, and none that runs a command

#### Scenario: An invalid status is refused

- **WHEN** a plugin asks for a task status the kernel does not define
- **THEN** the call fails naming the accepted statuses, and no record changes

### Requirement: A plugin asks the kernel to run an automation and never runs one

The run operation SHALL mark an automation due and return. The plugin's VM MUST
NOT execute the automation's action, spawn a process, open a session, or run a
shell command, and the host MUST NOT provide any binding by which it could.

The kernel's existing scheduler fires the automation on its next pass, under the
same claim that de-duplicates a running TUI and a headless tick — so a plugin
cannot cause a double fire, and cannot fire something at all when the automation
is not one the user authored.

Marking an automation due that is already due MUST be idempotent: it is the same
request, not a second run.

#### Scenario: A plugin runs an automation

- **WHEN** a plugin holding the automation-write capability runs an existing
  automation
- **THEN** the automation becomes due and the kernel fires it on its next pass

#### Scenario: The plugin's VM spawns nothing

- **WHEN** the run operation is performed
- **THEN** no process is started from the plugin's thread

#### Scenario: Marking a pending run again

- **WHEN** a plugin runs an automation twice before the kernel's next pass
- **THEN** the automation fires once

### Requirement: Enabling an automation follows the same rule the native pane follows

Setting an automation's enabled flag SHALL recompute its next occurrence from its
schedule when enabling, and clear it when disabling — the behaviour thurbox's own
automations pane has. The rule MUST have a single implementation shared by both
callers, so a plugin toggling an automation and a user toggling the same one
cannot leave different state.

#### Scenario: Enabling schedules the next occurrence

- **WHEN** a disabled automation with a recurring schedule is enabled through the
  plugin binding
- **THEN** its next occurrence is set, as it would be from the native pane

#### Scenario: Disabling clears the next occurrence

- **WHEN** an enabled automation is disabled through the plugin binding
- **THEN** it has no next occurrence and the scheduler skips it

### Requirement: A mutating binding is granted per record kind

Task writes and automation writes SHALL be separate declared capabilities.
Granting one MUST insert only its own bindings, and a plugin holding neither MUST
find no mutating binding at all. Denial MUST be by absence of the binding, never
by a binding that refuses.

Per kind rather than one write grant, for the reason the readers are split: the
declared set is what an install prompt is written from, and changing a user's task
list is a different disclosure from enabling and triggering their scheduled
automations.

A write capability MUST NOT imply the matching read capability, or the reverse: a
plugin that wants to draw the task list and a plugin that wants to close a task
are asking for different things.

#### Scenario: One write capability grants only its own bindings

- **WHEN** a plugin declares the task-write capability alone
- **THEN** its environment carries the task-write bindings and no automation-write
  binding

#### Scenario: No write capability grants nothing

- **WHEN** a plugin declares no write capability
- **THEN** its environment carries no mutating binding

#### Scenario: A read grant is not a write grant

- **WHEN** a plugin declares the capability that reads tasks and not the one that
  writes them
- **THEN** it can read the task list and no task-write binding is present

### Requirement: A plugin's write runs on the plugin's own thread and reaches the interface without new plumbing

A mutating binding SHALL execute on the thread that owns the calling VM, never on
the thread that draws frames, and MUST reach the database through its own
connection built on that thread.

A write MUST become visible to the interface through the change detection the
kernel already runs, and MUST NOT mark the interface dirty by itself — the
demand-driven paint gate is not something a plugin may reach.

A failure to reach storage MUST surface as an error to the calling plugin and MUST
NOT fail the host, the pane, or the frame.

#### Scenario: The frame thread is not involved

- **WHEN** a plugin performs a mutating operation
- **THEN** the write happens on the plugin's own thread

#### Scenario: The change appears in the interface

- **WHEN** a plugin changes a record the interface is displaying
- **THEN** the displayed record follows within the kernel's existing refresh
  cadence

#### Scenario: Storage is unavailable

- **WHEN** a mutating binding cannot reach storage
- **THEN** the call fails for the plugin, and the host keeps running

### Requirement: A plugin's write is recorded exactly where the kernel's own is

A mutation performed by a plugin SHALL go through the same storage operation the
kernel's own surface uses, so whatever that operation records — an audit entry for
a task, a run-history entry when an automation fires — is recorded for a plugin's
write too, with no separate plugin trail.

The host MUST NOT add auditing that the kernel's equivalent operation does not
already perform: a plugin-only trail would be a second description of what
changed, and the two would drift.

#### Scenario: A plugin's task deletion is audited

- **WHEN** a plugin deletes a task
- **THEN** the deletion appears in the audit trail, as it does when the native pane
  deletes one

#### Scenario: A plugin-triggered run appears in the run history

- **WHEN** a plugin marks an automation due and the kernel fires it
- **THEN** the run is recorded in that automation's run history, as any other run
  is
