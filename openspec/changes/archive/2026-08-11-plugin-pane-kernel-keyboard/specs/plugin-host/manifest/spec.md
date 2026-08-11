# plugin-host/manifest Specification

## ADDED Requirements

### Requirement: A pane may declare the kernel keyboard it is the pane for

A `[[panes]]` entry MAY name the kernel key context whose keyboard it answers,
spelled as the kernel spells that context (`key_context = "Tasks"`). Omitting it
MUST leave the pane exactly as panes were before: focusable only if its plugin
declared input, and answering only its own bindings.

The name SHALL be validated against a **closed set**: the contexts that scope a
*pane's* keyboard. A name that is no context at all, and a real context that scopes
no pane, MUST each be a manifest error naming the offending value and listing the
contexts that are accepted. The global context is refused because it belongs to no
pane, and the terminal context is refused because its keys are forwarded to a
process rather than dispatched as actions — a pane claiming it would receive
nothing and no error would say why.

Two panes in one manifest MUST NOT name the same keyboard: one keyboard belongs to
one pane, and a keypress that reached two of a plugin's own panes would have no
defined meaning.

A `[[keybindings]]` entry naming a pane that declared a keyboard MUST be a manifest
error. Such a pane answers thurbox's own actions; a binding of its own would be a
second answer to one keypress, and the host refuses the declaration rather than
silently preferring one.

#### Scenario: A pane names a pane keyboard

- **WHEN** a manifest declares a pane naming a context that scopes a pane's keyboard
- **THEN** the manifest validates and the pane carries that context

#### Scenario: A pane names something that is not a context

- **WHEN** a manifest declares a pane naming a key context the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names a context that scopes no pane

- **WHEN** a manifest declares a pane naming the global or the terminal context
- **THEN** validation fails naming the context and listing the contexts that are
  accepted

#### Scenario: Two panes name one keyboard

- **WHEN** a manifest declares two panes naming the same context
- **THEN** validation fails naming that context

#### Scenario: A pane with a keyboard also declares a binding

- **WHEN** a manifest declares a keybinding whose pane declared a kernel keyboard
- **THEN** validation fails naming the binding and the pane

#### Scenario: A pane names no keyboard

- **WHEN** a manifest declares a pane without naming a context
- **THEN** the manifest validates and the pane carries none
