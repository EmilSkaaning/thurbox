# migration/handover delta

## ADDED Requirements

### Requirement: A pane's window is converged before its handover, not during it

Where a native pane resolves which rows are on screen by a rule other than the kernel's
own, that window SHALL be converged in a change **before** its handover, and MUST NOT be
converged inside it.

The reason is what a handover is allowed to claim: that which code draws a pane changed
and nothing else about the pane did. A commit that also changes how the pane scrolls makes
that claim unverifiable, because every moved cell has two candidate causes and the
recorded expectation moves for two reasons at once. This is the frame rule applied to the
window — a window, like a frame, is a property of how the host draws a pane, and a
handover must not be able to change one under cover of moving the drawing code.

Convergence MUST run in the direction of the kernel's rule, never the other way.

The visible consequence SHALL be recorded as a decided behavioural change with its reason
— which rows are beside the cursor when the list overflows — rather than left to be
discovered in a frame diff.

#### Scenario: The window converges first

- **WHEN** a native pane whose window is its widget's is prepared for handover
- **THEN** the pane is changed to window by the kernel's rule in its own change, and the
  handover changes no window

#### Scenario: The changed scrolling is stated

- **WHEN** convergence changes which rows sit beside the cursor in an overflowing list
- **THEN** the change states that consequence and why the kernel's rule is the one both
  panes take, rather than leaving it to a diff

#### Scenario: Converging the rule to the pane is refused

- **WHEN** closing the difference by changing the kernel's helper to the pane's widget
  behaviour is proposed
- **THEN** it is refused, because the helper is shared and the pane is the thing being
  deleted
