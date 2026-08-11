# plugin-host/input Specification

## MODIFIED Requirements

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
