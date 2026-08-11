# phase-4 (delta)

## MODIFIED Requirements

### Requirement: The ported pane produces the native pane's view tree

The plugin SHALL produce a view tree **equal** to the one the native pane builds
from the same state, across a range of content variants including absent optional
sections. Equality of the tree is the check, because the same renderer paints
both — so an equal tree is a byte-identical pane without needing to compare
frames.

A divergence MUST be enumerated with its reason and pinned by its own test,
never absorbed by weakening the comparison.

**The comparison MUST also be able to outlive the native builder it names.** A
port's equality is written against an expression the eventual handover deletes, so
a port whose pane is a handover candidate SHALL additionally record the native
pane's tree as a checked-in expectation, and assert the plugin against that. A
port MUST NOT rely on the differential assertion alone as its handover evidence,
because the handover removes one side of it and what survives is a test that the
plugin renders without erroring — which a pane drawing entirely wrong rows also
satisfies.

#### Scenario: Trees agree for a fully populated pane

- **WHEN** the native pane and the plugin are given the same state, with every
  optional section present
- **THEN** the two view trees are equal

#### Scenario: Trees agree when optional sections are absent

- **WHEN** the same comparison is run with the optional sections omitted one at a
  time
- **THEN** the two view trees are equal in each case

#### Scenario: A divergence is pinned

- **WHEN** the plugin cannot reproduce some part of the native pane
- **THEN** a test asserts what it does instead and states why, and the
  comparison for every other case still demands equality

#### Scenario: The native builder is deleted by a later handover

- **WHEN** a pane's native builder is removed and only the plugin remains
- **THEN** a recorded expectation still constrains the pane's tree for every case
  the differential comparison covered

## ADDED Requirements

### Requirement: A recorded expectation is derived from the native pane and checked against it

A recorded expectation for a ported pane SHALL be generated from the **native**
builder, not from the plugin, and the change that records it MUST assert that the
native builder still reproduces it. While both sides exist, both edges MUST hold:
the recording equals the native tree, and the plugin equals the native tree.

A recording captured from the plugin — or captured after the native builder is
gone — MUST NOT be treated as an expectation, because it freezes whatever the
plugin does as correct, including a defect, and the resulting test can never fail
for the reason it exists.

Recording MUST therefore happen in a change that does not also delete the native
builder, so that the recording's provenance is demonstrable in the run that
introduces it.

#### Scenario: The recording is captured while both sides exist

- **WHEN** a recorded expectation is introduced for a ported pane
- **THEN** the native builder is still present, the recording equals its tree for
  every case, and the plugin equals its tree for every case

#### Scenario: A recording is proposed from the plugin's output

- **WHEN** a recorded expectation would be generated from the plugin rather than
  from the native builder
- **THEN** it is refused, because a plugin defect would become the expectation

#### Scenario: The plugin diverges after the recording exists

- **WHEN** the plugin's tree changes in any recorded respect
- **THEN** the recorded comparison fails and names the node that moved

### Requirement: A compact recorded expectation is exhaustive over the view tree

A recorded expectation MAY be a compact rendering rather than a full structural
dump, but its renderer SHALL destructure every view-tree variant and every style
field by name, with no rest pattern and no wildcard arm. Adding a field to the
view tree MUST fail to compile until the recording accounts for it.

A compact form is required to stay legible, because an expectation no reviewer can
read is one every update rubber-stamps — and a rubber-stamped expectation records
what the code last did rather than what the pane should show. Exhaustiveness is
what stops legibility from being bought with an omission: a fact absent from the
recording is a fact the oracle no longer constrains.

#### Scenario: A view-tree field is added

- **WHEN** a new field is added to a view-tree node or to a text style
- **THEN** the recording's renderer fails to compile until it prints or
  deliberately accounts for that field

#### Scenario: A style fact is set

- **WHEN** a node carries any non-default style fact
- **THEN** the recording shows that fact

#### Scenario: The recording stays reviewable

- **WHEN** a fully populated pane is recorded
- **THEN** the recording is a line-per-node rendering a reviewer can read, not a
  structural dump of every default-valued field
