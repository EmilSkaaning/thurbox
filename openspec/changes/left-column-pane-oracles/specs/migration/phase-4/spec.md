# phase-4 (delta)

## MODIFIED Requirements

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

**A pane whose handover is attempted SHALL have its recording before the attempt
concludes, whichever way it concludes.** A refused handover leaves the native
builder in place and produces no recording, so the pane's oracle keeps its
differential shape and the next attempt inherits the same hole — while the only
moment the recording can be *proven* to be the native pane's is one in which that
builder still exists. Recording is therefore owed by the attempt, not by the
handover.

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

#### Scenario: A handover attempt is refused

- **WHEN** an attempt to hand a pane over concludes that the native pane stays
- **THEN** the pane's recorded expectation exists and is checked against its
  native builder in the same change, so the next attempt does not start from a
  differential oracle again

## ADDED Requirements

### Requirement: One recorder serves every recorded pane

The renderer that produces a recorded expectation SHALL be single-sourced across
the panes that record one, rather than copied per pane.

The renderer's exhaustiveness over the view tree is the property that stops a
compact recording from silently omitting a fact, and that property is worth as
much as the number of copies of it: N copies are N formats that can drift, and a
field added to the view tree would have to be accounted for N times to keep the
oracle whole. Single-sourcing also makes two panes' recordings comparable, since
a difference between them is then a difference between the panes.

A test file MAY hold private helpers that read the source tree, which are
duplicated deliberately elsewhere in the suite; the constraint here is specific to
the renderer that defines what a recording *contains*.

#### Scenario: A second pane records its tree

- **WHEN** another ported pane gains a recorded expectation
- **THEN** it uses the existing recorder rather than its own copy

#### Scenario: A view-tree field is added while several panes record

- **WHEN** a field is added to a view-tree node or to a text style
- **THEN** exactly one place fails to compile, and fixing it restores every
  pane's recording at once
