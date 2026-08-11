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

**Every pane a bundled plugin reproduces SHALL carry a recording**, whether or not
its handover has been attempted. An earlier form of this requirement was owed by
an *attempt*: a pane whose handover was attempted had to be recorded before the
attempt concluded, either way. That trigger is too late and too weak. It is too
late because three panes' attempts concluded before the rule existed and inherited
the differential oracle it forbids; it is too weak because an attempt is a human
decision to begin work, so a pane nobody attempts keeps its differential oracle
indefinitely — and the change that eventually deletes the native builder is the
one least able to notice that the evidence went with it. Reproduction is the
earliest moment at which the recording is both **owed** and **provable**: the
plugin exists, so there is something to constrain, and the native builder exists,
so the baseline can be shown to be the pane's.

The requirement SHALL be enforced by the tree rather than by convention: a pane
whose oracle holds no recording MUST NOT be recorded handed over, so the native
builder stays protected until the recording exists. See
`migration/teardown`'s handover conditions.

A pane recorded structurally unportable, with no bundled plugin reproducing it, is
owed no recording — there is no plugin to constrain and no handover to gate.

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

#### Scenario: A pane is reproduced and never attempted

- **WHEN** a bundled plugin reproduces a native pane and no handover of it is
  attempted
- **THEN** the pane still carries a recording checked against its native builder,
  because reproduction is what makes the recording owed

#### Scenario: A handover attempt is refused

- **WHEN** an attempt to hand a pane over concludes that the native pane stays
- **THEN** the pane's recorded expectation exists and is checked against its
  native builder, so the next attempt does not start from a differential oracle
  again

#### Scenario: A pane's oracle is still differential when its handover is proposed

- **WHEN** a handover would delete a native builder that is the only right-hand
  side its pane's oracle has
- **THEN** the handover is refused by the teardown inventory, naming the missing
  recording, rather than being permitted and repaired by dropping the assertion

#### Scenario: A surface is recorded unportable

- **WHEN** a native surface is recorded as structurally unportable and no bundled
  plugin reproduces it
- **THEN** no recording is owed for it
