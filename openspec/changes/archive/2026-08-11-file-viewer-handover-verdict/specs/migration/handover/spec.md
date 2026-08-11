# migration/handover Specification

## ADDED Requirements

### Requirement: A refused handover records what it still needs, and what it does not

A handover that is proposed and refused SHALL record, as executable rows rather than
prose, one entry per thing it still needs — each re-derived from the source so it cannot
go stale.

A row whose requirement has **stopped being a requirement** MUST be re-verdicted rather
than deleted, and MUST keep asserting whatever half of it still matters. In particular,
where a handover was expected to need a **capability** and no longer does, the row MUST
assert that the capability is still **not** granted: otherwise the record of "the grant
was unnecessary" is indistinguishable from the grant having quietly happened.

The refusal MUST distinguish an unmade **decision** from a refused one. A decision the
host declines in principle blocks the pane; a decision nobody has taken yet blocks the
change, and a reader who cannot tell which is looking at the wrong problem.

#### Scenario: A requirement that stopped being one

- **WHEN** a route is added that makes a recorded requirement unnecessary
- **THEN** its row is re-verdicted with a probe deriving the new fact, and still asserts
  that the power it named was not granted

#### Scenario: The remainder is characterised

- **WHEN** the refusal is recorded
- **THEN** a rule asserts what kind of thing is outstanding, so "it needs a capability"
  cannot be inferred from a table where none of the rows is one
