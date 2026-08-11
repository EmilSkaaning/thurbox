# migration/phase-4 delta

## ADDED Requirements

### Requirement: A scheduled surface that the pane model cannot express is recorded, not approximated

When a surface this phase schedules cannot be reproduced by a plugin pane at all,
the port SHALL produce the record instead of the plugin: every host power the
surface needs and does not have, named individually with the reason it is
missing. No bundled plugin reproducing only the part the host can already express
MUST be shipped, because the phase measures the surface a third party gets and a
pane that cannot do the surface's job reports a capability the host does not have.

The record MUST separate a **vocabulary** gap — one the host would close with a
further node, style token or emphasis — from a **structural** one, where the
surface is not a pane. Only the first is closed by widening the catalogue, and a
change MUST NOT close a vocabulary gap for a pane it is not shipping.

#### Scenario: A surface is assessed as unportable

- **WHEN** a scheduled surface cannot be reproduced by a plugin pane
- **THEN** the change records each missing host power with its reason, ships no
  bundled plugin for that surface, and adds no capability, node, style token or
  pane slot

#### Scenario: The expressible part is not shipped as a gesture

- **WHEN** part of the surface's rendering could be expressed with today's
  catalogue
- **THEN** the record says so, and no pane is shipped that reproduces only that
  part

#### Scenario: The native surface is untouched

- **WHEN** such a change lands
- **THEN** the surface renders exactly as before and the teardown inventory still
  protects its renderer

### Requirement: Global search is recorded as structurally unportable

Global search SHALL be recorded as out of scope for the bundled-plugin phase on
structural grounds, not for want of vocabulary. The record MUST name at least
these four, each of which is a power the pane model withholds by design:

- the layout cannot seat a full-width band for a plugin — the pane-slot
  vocabulary is a closed set whose only member is the right-hand column;
- no capability publishes the query or its results, and none can be scoped
  honestly: computing the search requires reading every session's live terminal
  screen, while publishing the kernel's results would publish the strip's
  rendering rather than kernel state;
- the surface *produces* the restyling of rows in panes it does not own: a
  running search's verdict already reaches a plugin as a property of its own
  published rows, but a plugin's tree is painted into its own rect and nothing
  carries a query the other way;
- activating or previewing a result writes focus and another pane's cursor, and
  the kernel-state channel is read-only by construction.

The record MUST also name the vocabulary gaps separately, and MUST NOT close them
in the same change.

#### Scenario: The verdict is recorded with its reasons

- **WHEN** the phase's pane-readiness audit is read
- **THEN** global search's section states that it cannot be a plugin pane under
  this model and names each structural blocker and each vocabulary gap

#### Scenario: No bundled plugin claims the surface

- **WHEN** the bundled plugins are enumerated
- **THEN** none of them is a global-search pane

### Requirement: An unportability verdict is re-derived from the source

A recorded unportability verdict SHALL be re-derived from the source tree by a
test, so that closing one of its blockers fails the record rather than leaving it
to expire unnoticed. Each blocker MUST have its own probe, a probe MUST be scoped
to the declaration it reads rather than to a whole file, and a failure MUST name
the blocker whose verdict changed.

The verdict MUST NOT be merged into the teardown inventory, which answers a
different question — whether a native renderer may be deleted — and whose verdict
for the surface is unchanged either way.

#### Scenario: A blocker is closed later

- **WHEN** a host change closes one of the recorded blockers
- **THEN** the test fails and names it, so the verdict is revisited in the change
  that closed it

#### Scenario: The verdict still holds

- **WHEN** nothing relevant has changed
- **THEN** the test passes, and it also asserts that no bundled plugin claims the
  surface

#### Scenario: The teardown inventory is unaffected

- **WHEN** the teardown inventory is checked after the record lands
- **THEN** the surface's native renderer is still required to exist, for the same
  reason as before
