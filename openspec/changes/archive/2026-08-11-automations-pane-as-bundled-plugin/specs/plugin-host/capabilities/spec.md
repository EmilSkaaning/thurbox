# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Reading scheduled automations is one capability with two readers

Reading thurbox's automations SHALL require the automations capability, and that one
capability SHALL grant **both** views the host publishes of them: the filtered list
of automations due to fire, and the full pane list of every automation with its
schedule, its state and the cursor's position in it.

One capability rather than two, because the declared set is what an install prompt
is written from and both readers answer the same sentence — "reads the automations
you have scheduled". A second capability would ask a user to distinguish "the due
ones" from "all of them", which is not a distinction a user is protected by.

A plugin that declares it MUST receive both readers and no other state reader; a
plugin that declares another state capability MUST NOT receive either automations
reader. Enforcement stays by absence: an undeclared reader is not inserted, rather
than being present and refusing.

#### Scenario: The capability grants both readers

- **WHEN** a plugin's manifest declares the automations capability alone
- **THEN** both automations readers are present in its module table and the session,
  metrics, task and file readers are absent

#### Scenario: Another state capability grants neither

- **WHEN** a plugin declares only the session capability
- **THEN** both automations readers are absent from its module table

#### Scenario: The capability counts as reading kernel state

- **WHEN** the only running plugin declares the automations capability
- **THEN** the host reports that kernel state has a reader, so the publisher builds
  a snapshot
