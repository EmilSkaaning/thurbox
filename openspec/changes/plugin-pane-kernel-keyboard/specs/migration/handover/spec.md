# migration/handover Specification

## ADDED Requirements

### Requirement: A handed-over pane with a keyboard keeps that keyboard

A pane whose native counterpart had a scoped keyboard SHALL keep it: its manifest
MUST declare the key context the native pane was scoped to, so every action of that
context still resolves while the pane holds focus, still fires against the kernel's
own state, and is still rebindable in the keybinding editor and persisted to the
user's keybindings file.

The keyboard MUST NOT be re-implemented in the plugin. A pane's keys operate on
kernel state — a cursor, a record, a directory listing, an editor process — and
delivering them to a plugin would require granting each of those as a capability,
which is a wider surface than the pane needs and a different pane's behaviour than
the one being replaced.

The pane MUST be focusable through the same entry as the native pane: the focus
cycle stop, or the hand-off keys of the column it sits in, available whenever the
pane is on screen rather than depending on which code draws it.

#### Scenario: A scoped key still works after handover

- **WHEN** a key bound to one of the replaced pane's scoped actions is pressed while
  the replacement holds focus
- **THEN** the action fires exactly as it did before the handover

#### Scenario: The keyboard is still rebindable

- **WHEN** a user rebinds one of that context's actions in the keybinding editor
- **THEN** the new chord drives the replacement pane, and the change is persisted

#### Scenario: Focus reaches the replacement

- **WHEN** the focus cycle is stepped while the replacement is on screen
- **THEN** focus lands on it, reported as the pane it replaced
