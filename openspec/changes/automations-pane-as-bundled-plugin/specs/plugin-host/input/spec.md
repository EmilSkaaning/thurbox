# plugin-host/input Specification

## ADDED Requirements

### Requirement: A plugin pane's cursor is the plugin's own

A pane granted input SHALL be able to own the cursor its keys move. The host
publishes the cursor of the *native* surface a pane may be reproducing; that value
is a starting point, not the pane's cursor, and a plugin MUST be able to hold a
cursor of its own across renders and act on the row it names.

This follows from the runtime rather than from a new binding: one VM per plugin,
retained across render and key calls, so a cursor is ordinary plugin state. The host
MUST NOT require a view-write capability for it, because moving a plugin's own cursor
is not a write to anything the kernel owns — the kernel's cursor is untouched, and a
pane that only draws is unaffected.

A pane that acts on a record MUST address it by the id the published row carries,
not by an index into the kernel's list, so that the row acted on is the row the pane
drew.

#### Scenario: A key moves the plugin's cursor and not the kernel's

- **WHEN** a pane holding input receives its movement key
- **THEN** the row its own list names as selected changes, and the published section's
  cursor is unchanged

#### Scenario: The plugin's cursor survives the next render

- **WHEN** the pane is rendered again after the key
- **THEN** the tree still names the moved row, so the pane's cursor is not reset by
  the render that draws it

#### Scenario: A pane nobody has driven reproduces the published cursor

- **WHEN** a pane holding input has received no key yet
- **THEN** its list names the cursor the host published, so a copy of a native pane
  is identical until a user drives it

### Requirement: A pane is told nothing about its own focus

The host SHALL NOT tell a plugin whether the pane it is rendering holds focus. A
focus fact carried by a published state section describes the **native** surface that
section is about, and a plugin MUST NOT read it as a statement about its own pane.

The consequence MUST be recorded rather than hidden: a pane that would draw its
cursor only while focused cannot, and a pane cannot observe focus leaving it. The one
signal it does have is that a key arrived, which only a focused pane receives.

Because of that, a pane that changes records SHALL treat "no cursor is drawn" as
"nothing is selected" and refuse the change, rather than acting on the cursor the
host published for the native surface — which is a row the user of the plugin's pane
cannot see.

#### Scenario: The published focus is about the native pane

- **WHEN** a plugin pane holds focus while the native pane it reproduces does not
- **THEN** the published section reports the native pane unfocused, and the plugin's
  own focus is not reported anywhere

#### Scenario: An unseen row is not acted on

- **WHEN** a pane holding a write capability receives a record-changing key while no
  cursor is drawn in it
- **THEN** nothing is written, and the pane reports the key unconsumed

#### Scenario: A pane cannot observe focus leaving

- **WHEN** focus moves off a plugin pane that had received keys
- **THEN** the plugin is not called, so anything it drew because it had been driven
  is still drawn
