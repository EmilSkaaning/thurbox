# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: A published row of a writable record carries that record's id

Each published task row and each published automation row SHALL carry the record's
own id. Without it a pane that may change a record could not name the row it drew,
and would have to match on a title it composed itself — a title the kernel
publishes unfitted and a pane is free to alter.

The id MUST be published whether or not the reading plugin holds a write
capability, because the snapshot is one shape for every reader and a per-grant
snapshot would make what a pane sees depend on what it may do.

Publishing an id MUST NOT be read as publishing a handle: it is the same opaque row
number `thurbox-cli` already prints, it grants nothing on its own, and every change
made with it still requires the capability that permits that change.

#### Scenario: A task row carries its id

- **WHEN** a plugin holding the task capability reads the task list
- **THEN** each entry carries the task's id alongside its title and status

#### Scenario: An automation row carries its id

- **WHEN** a plugin holding the automation capability reads the scheduled
  automations
- **THEN** each entry carries the automation's id

#### Scenario: The id crosses without a write capability

- **WHEN** a plugin declares only the read capability for tasks
- **THEN** the rows still carry their ids, and no mutating binding is present
