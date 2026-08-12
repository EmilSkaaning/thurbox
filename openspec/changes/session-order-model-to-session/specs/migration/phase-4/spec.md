# migration/phase-4 Specification (delta)

## ADDED Requirements

### Requirement: The session list's ordering model is owned by the kernel, not by its pane

The order thurbox navigates its sessions in SHALL live in the pure-data layer, not in the
module that draws the session list. This covers the comparator the session-list pane and
`Ctrl+J`/`Ctrl+K` navigation share, the nesting of a child under its parent within a repo
group, the repo grouping keys and labels, the reorder primitive behind
`Shift+J`/`Shift+K`, the alphabetical sort behind `Shift+S`, the fuzzy match positions
global search resolves per session, and the width-free resolution of a rendered row.

The requirement is independent of any handover. A comparator that keyboard navigation
depends on is model, and a rendering module that also holds it makes the coordinator
depend on the layer that is supposed to depend on *it*.

The relocation SHALL NOT change behaviour: no relocated function's body is edited, no
signature is widened, and every caller is updated to name the model where the model now
lives.

Geometry SHALL stay behind. The fit of an agent's reported text into the columns a row has
left is a function of a resolved pane width, so it remains with the pane; the relocated
row type carries that field unset, and the pane fills it.

#### Scenario: Navigation names its own model

- **WHEN** the coordinator orders sessions for `Ctrl+J`, reorders them for `Shift+J`, or
  sorts them for `Shift+S`
- **THEN** it calls the pure-data layer, and no call from the coordinator into the
  rendering layer remains for any of the three

#### Scenario: The pane keeps its geometry

- **WHEN** the pane resolves how much of an agent's text fits after a session's name
- **THEN** that fit stays in the pane's module, and the relocated resolution leaves the
  fitted field unset

#### Scenario: The relocation is behaviour-preserving

- **WHEN** the model has moved
- **THEN** the pane draws the same frame, the recorded pane oracle is unchanged, and no
  snapshot moves

### Requirement: A refused pane's remaining structural blocker is stated with a measurement

Where a pane's handover is refused and one structural row remains, the change that closes
the others SHALL record **why the remaining row is not closable by it**, as a measurement
of the two behaviours that disagree rather than as a restatement of the row.

A row asserting that two rules "differ" is checkable only in the sense that both rules
exist. What decides whether a handover would regress is the size and shape of the
disagreement, and that is a fact a reader cannot re-derive from prose.

Where the row names more than one behaviour, the measurement MUST cover the ones a policy
choice would not settle, so that "pick the other scroll rule" is visibly not an answer.

#### Scenario: The remaining row is measured

- **WHEN** the last structural row is left open
- **THEN** the change records the two windows' actual output over a list longer than the
  pane, and the reader can see which rows each would show

#### Scenario: The measurement covers what policy cannot settle

- **WHEN** the row also names row granularity and the index space a click resolves through
- **THEN** those are stated as separate findings, because neither is decided by which
  scroll rule is chosen
