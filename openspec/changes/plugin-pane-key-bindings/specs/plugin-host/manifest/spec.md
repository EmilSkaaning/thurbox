# plugin-host/manifest Specification

## ADDED Requirements

### Requirement: A keybinding declaration names its pane, its chord and its capability

A keybinding declaration SHALL carry a stable `id`, the `pane` it is scoped to, an
optional human-readable `title`, and an optional default `chord`. The manifest
MUST be rejected when:

- the `pane` names no pane the same manifest declares — the binding would be
  scoped to nothing;
- the `chord` cannot be parsed by the keymap's chord grammar — the same grammar
  the user keybindings file uses, so a chord means one thing everywhere;
- the manifest declares a keybinding without requesting the capability to receive
  input — the binding could never be delivered.

Each rejection MUST name the offending binding, mirroring the rejection of a pane
declared without the capability to render: a declaration the host would never act
on fails where the error names its own fix, rather than becoming a key that
silently does nothing.

A declaration with **no** chord MUST be valid: it is how a plugin ships an action
without claiming a key, leaving the user to bind it.

#### Scenario: A binding names an unknown pane

- **WHEN** a manifest declares a keybinding whose pane it does not declare
- **THEN** validation fails naming the binding and the pane

#### Scenario: A binding declares an unparsable chord

- **WHEN** a manifest declares a keybinding whose chord the grammar does not
  accept
- **THEN** validation fails naming the binding and the chord

#### Scenario: A binding without the input capability

- **WHEN** a manifest declares a keybinding and does not request the input
  capability
- **THEN** validation fails naming the binding and the missing capability

#### Scenario: A binding with no chord

- **WHEN** a manifest declares a keybinding with a pane and no chord
- **THEN** the manifest validates and the binding is registered unbound

#### Scenario: A well-formed binding

- **WHEN** a manifest declares a pane, the render and input capabilities, and a
  keybinding naming that pane with a parsable chord
- **THEN** the manifest validates and the declaration carries all four fields
