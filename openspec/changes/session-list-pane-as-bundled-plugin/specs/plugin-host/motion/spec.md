# plugin-host/motion Specification

## ADDED Requirements

### Requirement: A native pane's animation is declared motion resolved through the frame table

A native pane that animates SHALL express its animation as a declared motion node
in its own view tree, and MUST resolve which frame is showing by reading a frame
table rather than by holding a clock of its own.

This keeps the renderer free of any path back to a plugin: the frame table is
plain data that the application layer fills before a paint, so the module that
draws an animation still cannot call the code that declared one. It also makes a
native pane and a plugin reproducing it comparable — the two trees hold the same
motion node, so a port's tree-equality claim covers the animated part of the pane
instead of exempting it.

The frames a native pane's motion declares MUST come from the same clock the pane
animated on before, so expressing an existing animation this way changes no
observable behaviour: the same frames at the same rate, and the same answer when
motion is reduced.

#### Scenario: A native pane animates

- **WHEN** a native pane's tree carries a motion node and the application fills
  the frame table with that node's current frame
- **THEN** the pane paints that frame, and paints frame zero when the table names
  no frame for it

#### Scenario: A plugin reproduces an animated native pane

- **WHEN** a bundled plugin declares the same motion — same frames, same rate,
  same key — as the native pane it reproduces
- **THEN** the two view trees are equal, including their motion nodes

#### Scenario: Reduced motion

- **WHEN** motion is reduced
- **THEN** the native pane's frame table resolves to frame zero, which is the same
  answer the kernel gives a plugin's declared motion

### Requirement: A motion's key is identity within its pane, not a name to be guessed

A motion's key SHALL identify the node **within its own pane**, so two panes may
use the same key without sharing phase and a plugin may choose any key it likes
without coordinating with the kernel or with another plugin.

Consequently a test that compares a plugin's tree against a native pane's may
require the two to use the same key, and MUST record that the requirement is an
artifact of comparing trees rather than a name a plugin has to know.

#### Scenario: Two panes use the same key

- **WHEN** two panes each declare a motion under the same key
- **THEN** each pane's animation keeps its own phase
