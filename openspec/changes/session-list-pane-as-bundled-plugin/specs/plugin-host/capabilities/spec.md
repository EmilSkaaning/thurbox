# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: The sessions capability covers every session, not only the active one

The capability that reads sessions SHALL gate every reader over session records —
both the one that answers about the session the user is on and the one that
returns the whole rendered list. A plugin declaring it MUST receive both; a plugin
that does not declare it MUST receive neither, enforced by the bindings' absence
rather than by a check inside them.

One capability rather than two, because the declared set is what an install prompt
is written from and both readers answer the same question a user is being asked:
*may this plugin see your sessions?* Splitting them would put two questions in the
prompt for one disclosure and would make a pane that draws the session list demand
two grants to draw one pane.

The capability's documented sentence MUST state that the grant covers every
session and not only the active one, because the disclosure genuinely widens —
one session's name and activity text becomes every session's — and a capability
list is only honest if it says what it discloses.

#### Scenario: A plugin declares the sessions capability

- **WHEN** a plugin's manifest declares the sessions capability
- **THEN** both the active-session reader and the session-list reader are present
  in its module table

#### Scenario: A plugin declares another state capability

- **WHEN** a plugin declares a kernel-state capability other than the sessions one
- **THEN** neither session reader is present in its module table

#### Scenario: A pane needs no second grant

- **WHEN** a plugin declares only rendering and the sessions capability
- **THEN** it can draw the whole session list, and its recorded grant set names
  exactly those two powers
