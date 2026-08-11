# plugin-host/input Specification

## MODIFIED Requirements

### Requirement: Only a plugin that declared input is focusable

A plugin pane SHALL be focusable only if it can receive keys, and there are exactly
two ways it can: its plugin declared the input capability, or the pane declared one
of the kernel's own pane keyboards. A pane with neither MUST be skipped by focus
navigation and MUST never be handed a key.

When several focusable plugin panes are on screen, focus SHALL name **which** pane
holds it, and every input the host delivers — a key or a click — MUST go to that
pane. Focus that named only "a plugin pane" would send every key to the first one
declared, so a second focusable pane could never be used.

A pane that declared a kernel keyboard SHALL be focused **as thurbox's own pane of
that name**, not as "a plugin pane": the scoped actions of that context resolve
while it holds focus, the kernel dispatches them against its own state, and the
plugin is never handed the key. Its plugin therefore needs no capability to be
driven, and gains no power by being driven. Focus entry MUST be available whenever
either occupant of that pane's place is on screen, so the ring stop and the
hand-off keys do not depend on which code is drawing.

A pane that stops being focusable while it holds focus — it was hidden, its plugin
was reloaded without it, or its plugin stopped — MUST NOT keep it: the host falls
back to another focusable pane, or to none.

#### Scenario: A pane without the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin lacks the input
  capability and the pane declared no kernel keyboard
- **THEN** focus skips it

#### Scenario: A pane with the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin declared input
- **THEN** focus can land on it

#### Scenario: A pane that declared a kernel keyboard

- **WHEN** focus is cycled and a visible plugin pane declared a kernel pane keyboard
  while its plugin declared no input capability
- **THEN** focus can land on it, and it is reported as thurbox's own pane of that
  name

#### Scenario: A scoped action fires in such a pane

- **WHEN** a key bound to one of that context's scoped actions is pressed while the
  pane holds focus
- **THEN** the kernel performs the action against its own state, and the plugin is
  not called

#### Scenario: Two focusable panes

- **WHEN** two focusable plugin panes are on screen and focus is on the second
- **THEN** a key is delivered to the second, not to the first

#### Scenario: The focused pane disappears

- **WHEN** the pane holding focus is hidden or its plugin stops
- **THEN** focus does not stay on it, and no key is delivered to it

### Requirement: A pane is told nothing about its own focus

The host SHALL NOT tell a plugin whether the pane it is rendering holds focus, for
every pane that is a **reproduction**: a focus fact carried by a published state
section describes the native surface that section is about, and such a plugin MUST
NOT read it as a statement about its own pane.

The exception is a pane that declared that surface's own keyboard. There the two are
the same pane — the section's focus fact reports which pane of the interface the
keyboard is on, and that pane is the plugin's — so the flag MUST be true when it
holds focus. This is a consequence of the focus, not a new fact published: no
section gains a field, and a pane that declares no keyboard sees exactly what it saw
before.

The consequence for a reproduction MUST be recorded rather than hidden: a pane that
would draw its cursor only while focused cannot, and a pane cannot observe focus
leaving it. The one signal it does have is that a key arrived, which only a focused
pane receives.

Because of that, a pane that changes records SHALL treat "no cursor is drawn" as
"nothing is selected" and refuse the change, rather than acting on the cursor the
host published for the native surface — which is a row the user of the plugin's pane
cannot see.

#### Scenario: The published focus is about the native pane

- **WHEN** a plugin pane that declared no kernel keyboard holds focus while the
  native pane it reproduces does not
- **THEN** the published section reports the native pane unfocused, and the plugin's
  own focus is not reported anywhere

#### Scenario: The published focus is about a pane that owns the keyboard

- **WHEN** a plugin pane that declared a kernel pane keyboard holds focus
- **THEN** the published section for that surface reports it focused

#### Scenario: An unseen row is not acted on

- **WHEN** a pane holding a write capability receives a record-changing key while no
  cursor is drawn in it
- **THEN** nothing is written, and the pane reports the key unconsumed

#### Scenario: A pane cannot observe focus leaving

- **WHEN** focus moves off a plugin pane that had received keys
- **THEN** the plugin is not called, so anything it drew because it had been driven
  is still drawn
