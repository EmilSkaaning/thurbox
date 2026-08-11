# plugin-host/input Specification

## MODIFIED Requirements

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
