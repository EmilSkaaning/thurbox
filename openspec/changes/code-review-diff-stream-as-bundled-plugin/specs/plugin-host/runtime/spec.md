# plugin-host/runtime Specification

## ADDED Requirements

### Requirement: A plugin may walk a string by character

A plugin's VM SHALL be created with the standard library that decodes and encodes
UTF-8, so plugin code can iterate a string's **characters** rather than its bytes.

It is admissible under the restricted-environment rule because it grants no
ambient access of any kind: it reaches no file, no process, no environment
variable and no clock. It is pure computation over a string the host already
handed the plugin.

It is necessary rather than convenient. A pane that styles the *inside* of a line
— highlighting code, splitting a matched run, measuring an indent — must agree
with the host about where one character ends and the next begins, and the host
counts characters. Without it a plugin scanning a line containing any multi-byte
character drifts after the first one and every run to its right is wrong, which is
a silently incorrect pane rather than a refused one.

#### Scenario: A plugin iterates a multi-byte string

- **WHEN** plugin code walks a string containing multi-byte characters
- **THEN** it visits one character per iteration and can rebuild each as a string

#### Scenario: The addition grants no ambient access

- **WHEN** the plugin environment is enumerated
- **THEN** it still contains no filesystem, process, environment or clock library,
  and the addition is a pure-computation library like the arithmetic one beside it
