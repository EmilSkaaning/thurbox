# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Changing the task list is its own declared capability

The closed capability vocabulary SHALL include a capability that permits changing
tasks, separate from the one that reads them. A plugin declaring it MUST receive
the task-write bindings and no other mutating binding; a plugin that has not
declared it MUST find no task-write binding at all.

It MUST be separate from the read capability in both directions. Reading the task
list to draw it and changing what is in it are different disclosures, and a pane
that only draws must not have to ask for the power to delete.

Its documented sentence MUST state that it changes the user's own records, because
a capability list is only honest if it says what it does — every other capability
in the vocabulary is a read, and this is the first that is not.

#### Scenario: The write capability grants exactly its own bindings

- **WHEN** a plugin declares only the task-write capability
- **THEN** its environment carries the task-write bindings and no automation-write
  binding

#### Scenario: The read capability does not imply the write one

- **WHEN** a plugin declares only the capability that reads tasks
- **THEN** no task-write binding is present in its environment

#### Scenario: The write capability does not imply the read one

- **WHEN** a plugin declares only the task-write capability
- **THEN** no task reader is present in its environment

### Requirement: Changing automations is its own declared capability

The vocabulary SHALL include a capability that permits enabling, running and
deleting automations, separate from the one that reads them and separate from the
task-write capability.

Its documented sentence MUST state that an automation the user authored may run a
program, and that this capability can cause one to run — the reach a user is being
asked about is not "edits a list", and the capability list is what an install
prompt is written from.

It MUST NOT grant the power to author or edit an automation, so the set of actions
a plugin holding it can cause is exactly the set the user already scheduled.

#### Scenario: The capability grants exactly its own bindings

- **WHEN** a plugin declares only the automation-write capability
- **THEN** its environment carries the automation-write bindings and no task-write
  binding

#### Scenario: It authors nothing

- **WHEN** a plugin holding the automation-write capability inspects its
  environment
- **THEN** there is no binding that creates an automation, edits one's schedule or
  action, or runs a command of the plugin's choosing

#### Scenario: No write capability at all

- **WHEN** a plugin declares neither write capability
- **THEN** no mutating binding is present in its environment
