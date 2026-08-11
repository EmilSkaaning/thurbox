# plugin-host/input Specification

## Purpose
Defines how a plugin pane receives keyboard input — when it is focusable, what
the plugin is told, and what happens to keys it does not want.
## Requirements
### Requirement: Only a plugin that declared input is focusable

A plugin pane SHALL be focusable only if its plugin declared the input
capability. A pane without it MUST be skipped by focus navigation and MUST
never be handed a key.

When several focusable plugin panes are on screen, focus SHALL name **which** pane
holds it, and every input the host delivers — a key or a click — MUST go to that
pane. Focus that named only "a plugin pane" would send every key to the first one
declared, so a second focusable pane could never be used.

A pane that stops being focusable while it holds focus — it was hidden, its plugin
was reloaded without it, or its plugin stopped — MUST NOT keep it: the host falls
back to another focusable pane, or to none.

#### Scenario: A pane without the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin lacks the input
  capability
- **THEN** focus skips it

#### Scenario: A pane with the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin declared input
- **THEN** focus can land on it

#### Scenario: Two focusable panes

- **WHEN** two focusable plugin panes are on screen and focus is on the second
- **THEN** a key is delivered to the second, not to the first

#### Scenario: The focused pane disappears

- **WHEN** the pane holding focus is hidden or its plugin stops
- **THEN** focus does not stay on it, and no key is delivered to it

### Requirement: A focused pane receives keys

While a plugin pane is focused, the host SHALL pass each key to its plugin and
MUST report to the plugin which key it was.

When the keypress resolved to one of that pane's registered bindings, the host
MUST also report **which binding** it resolved to, and MUST report nothing in its
place when it resolved to none. A plugin can then act on the binding it declared
rather than on the chord a user happens to have bound to it, so a rebind needs no
change to plugin code.

The raw key MUST still be reported in either case: a pane that collects text
needs the keypress even when a binding also matched, and a plugin that declares no
binding at all keeps working exactly as before.

#### Scenario: A key reaches the plugin

- **WHEN** a key is pressed while a plugin pane is focused
- **THEN** the plugin's key handler is called with that key

#### Scenario: The key resolved to a binding

- **WHEN** a key bound to one of the focused pane's bindings is pressed
- **THEN** the handler is called with both the key and that binding's id

#### Scenario: The key resolved to no binding

- **WHEN** a key that no binding of the focused pane holds is pressed
- **THEN** the handler is called with the key and no binding

#### Scenario: No key handler

- **WHEN** the plugin declares input but defines no key handler
- **THEN** the key is not consumed and nothing fails

### Requirement: The plugin decides whether a key is consumed

A plugin's key handler SHALL return whether it consumed the key. An unconsumed
key MUST fall through to thurbox's own handling, so a pane cannot swallow the
keys a user needs to get out of it.

#### Scenario: The plugin consumes the key

- **WHEN** the handler reports the key consumed
- **THEN** thurbox does not also act on it

#### Scenario: The plugin ignores the key

- **WHEN** the handler reports the key not consumed
- **THEN** thurbox handles it as it normally would

#### Scenario: The handler fails

- **WHEN** the handler raises or exceeds its budget
- **THEN** the key is treated as unconsumed and the failure is recorded against
  the plugin

### Requirement: Input never blocks the UI

Passing a key to a plugin SHALL NOT block the thread that draws frames. A
plugin that hangs in its key handler MUST NOT freeze the UI.

#### Scenario: A key handler hangs

- **WHEN** a plugin's key handler does not return
- **THEN** the UI continues to draw and accept input

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

