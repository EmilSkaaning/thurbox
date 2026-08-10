# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A ported pane is reachable from the keyboard

A pane reproduced as a bundled plugin SHALL be reachable from the keyboard
without knowing its plugin's name — a user MUST be able to put it on screen with
the bound pane-visibility action alone. A port MUST NOT rely on a headless
command or a stored choice as the only way to see the pane it added, because a
pane nobody can open is not evidence that the pane was ported.

This holds however many bundled panes exist, so it cannot regress as later panes
are added.

#### Scenario: The newest bundled pane can be shown

- **WHEN** the pane-visibility action is used with every bundled pane declared
- **THEN** each declared pane, including the most recently added one, can be put
  on screen and taken off again

#### Scenario: Reachability does not depend on declaration order

- **WHEN** a second bundled plugin declares a pane after an existing one
- **THEN** the later pane is as reachable as the first
