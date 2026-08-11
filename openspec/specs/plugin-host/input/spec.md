# plugin-host/input Specification

## Purpose
Defines how a plugin pane receives keyboard input — when it is focusable, what
the plugin is told, and what happens to keys it does not want.
## Requirements
### Requirement: Only a plugin that declared input is focusable

A plugin pane SHALL be focusable only if its plugin declared the input
capability. A pane without it MUST be skipped by focus navigation and MUST
never be handed a key.

#### Scenario: A pane without the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin lacks the input
  capability
- **THEN** focus skips it

#### Scenario: A pane with the input capability

- **WHEN** focus is cycled and a visible plugin pane's plugin declared input
- **THEN** focus can land on it

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

