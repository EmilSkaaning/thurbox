# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Reading the task list is its own capability

The host SHALL gate the task-list reader behind its own declared capability,
separate from every other kernel-state capability. A plugin that declares it
MUST receive the task reader and no other state reader; a plugin that does not
declare it MUST NOT receive the task reader at all — enforced by the binding's
absence, not by a check inside it.

Its own capability rather than a shared one, for the reason the state
capabilities are already split per kind: the declared set is what an install
prompt is written from, and "reads your task list" is a different question to ask
a user from "reads your sessions" or "reads this machine's CPU and memory".

#### Scenario: A plugin declares only the task capability

- **WHEN** a plugin's manifest declares the task capability alone
- **THEN** the task reader is present in its module table and the session,
  metrics and automation readers are absent

#### Scenario: A plugin declares another state capability

- **WHEN** a plugin declares a state capability other than the task one
- **THEN** the task reader is absent from its module table

#### Scenario: The task capability counts as reading kernel state

- **WHEN** the only running plugin declares the task capability
- **THEN** the host reports that kernel state has a reader, so the publisher
  builds a snapshot
