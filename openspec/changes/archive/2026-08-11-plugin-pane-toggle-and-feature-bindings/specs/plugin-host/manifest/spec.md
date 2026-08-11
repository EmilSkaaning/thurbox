# plugin-host/manifest Specification

## ADDED Requirements

### Requirement: A pane may declare the kernel action that toggles it

A `[[panes]]` entry MAY name the kernel action that shows and hides it, spelled as
the user keybindings file spells that action — one spelling for an action wherever a
user meets it. Omitting it MUST leave the pane reachable only through the generic
plugin-pane toggle, exactly as before.

The name SHALL be validated against a **closed set**: the actions whose purpose is
to show or hide a pane. A name that is not an action at all, an action that is not
one of those, and the generic plugin-pane toggle itself MUST each be a manifest
error naming the offending value and the actions that are accepted — the generic
toggle because it already reaches every declared pane, so binding it would toggle a
pane twice.

Two panes in one manifest MUST NOT name the same action: one key flipping two of a
plugin's own panes together is a declaration the host refuses rather than honours.

#### Scenario: A pane names a pane-toggle action

- **WHEN** a manifest declares a pane naming an action whose job is to show or hide
  a pane
- **THEN** the manifest validates and the pane carries that action

#### Scenario: A pane names something that is not an action

- **WHEN** a manifest declares a pane naming an action the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names an action that is not a pane toggle

- **WHEN** a manifest declares a pane naming a real kernel action whose job is not
  showing or hiding a pane
- **THEN** validation fails naming the action and listing the actions that are
  accepted

#### Scenario: A pane names the generic plugin-pane toggle

- **WHEN** a manifest declares a pane naming the action that already toggles every
  declared pane
- **THEN** validation fails, because the pane would be toggled twice

#### Scenario: Two panes name one action

- **WHEN** a manifest declares two panes naming the same action
- **THEN** validation fails naming that action

#### Scenario: A pane names no action

- **WHEN** a manifest declares a pane without naming an action
- **THEN** the manifest validates and the pane carries none

### Requirement: A pane may declare the feature flag that gates it

A `[[panes]]` entry MAY name the whole-feature switch that gates it, spelled as the
settings file spells that switch. Omitting it MUST mean the pane is gated by no
feature.

The name SHALL be validated against a **closed set** of the switches that exist. An
unrecognized switch MUST be a manifest error naming it, never a silently ignored
field — a pane gated on a flag the host does not have would be a pane that either
never appears or is never gated, and the manifest cannot say which was meant.

#### Scenario: A pane names an existing switch

- **WHEN** a manifest declares a pane naming a feature switch the host defines
- **THEN** the manifest validates and the pane carries that switch

#### Scenario: A pane names an unknown switch

- **WHEN** a manifest declares a pane naming a switch the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names no switch

- **WHEN** a manifest declares a pane without naming a switch
- **THEN** the manifest validates and the pane is gated by no feature
