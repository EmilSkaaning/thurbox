# migration/phase-4 delta

## ADDED Requirements

### Requirement: A divergence closed by giving the plugin a predicate keeps its measurement

Where a reproduction diverges from its native pane because the plugin's language cannot
express a rule the kernel applies, the divergence SHALL be closed by giving the plugin
**the predicate**, not the answer — and the port's enumerated case SHALL be inverted
rather than deleted.

Giving the answer is the tempting closure and it is refused: publishing the resolved
text moves a presentation decision into the publication and leaves the next pane wanting
the same rule exactly where this one was.

The inverted case MUST keep a guard that its fixture still exercises the difference. A
case that once asserted an inequality and now asserts an equality can pass by having
stopped testing anything, so it MUST also assert that the input still differs from its
resolved form.

#### Scenario: The predicate closes the divergence

- **WHEN** a reproduction cannot apply a kernel rule because its language has no
  equivalent
- **THEN** the host exposes the rule as a pure function, and the pane goes on deciding
  where to apply it

#### Scenario: The enumerated case inverts

- **WHEN** such a divergence closes
- **THEN** the port's inequality assertion becomes an equality assertion under a name
  saying so, and the gate row is re-verdicted with its probe re-derived

#### Scenario: The inverted case still exercises its fixture

- **WHEN** an inverted case runs
- **THEN** it asserts that the fixture's input still differs from its resolved form, so
  an equality cannot pass by testing nothing
