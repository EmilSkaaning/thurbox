# migration/handover delta

## MODIFIED Requirements

### Requirement: A pane's window is converged before its handover, not during it

Where a native pane resolves which rows are on screen by a rule other than the kernel's
own, that window SHALL be converged in a change **before** its handover, and MUST NOT be
converged inside it.

The rule the pane resolves its window by MAY be a list widget's stored offset or the
pane's own inline arithmetic; the two are the same problem and take the same answer. What
makes a window a convergence problem is that it is **not the kernel's**, not that a widget
owns it — so a pane that computes `start` and `end` itself SHALL converge exactly as one
that reads an offset back from a widget, and the size of the change MUST NOT be a reason
to fold it into the handover.

The reason is what a handover is allowed to claim: that which code draws a pane changed
and nothing else about the pane did. A commit that also changes how the pane scrolls makes
that claim unverifiable, because every moved cell has two candidate causes and the
recorded expectation moves for two reasons at once. This is the frame rule applied to the
window — a window, like a frame, is a property of how the host draws a pane, and a
handover must not be able to change one under cover of moving the drawing code.

Convergence MUST run in the direction of the kernel's rule, never the other way.

Where a **surface is drawn as several panes** — a diff and the changed-files list beside
it, each with its own focus and its own keys — each pane's window SHALL be converged
separately, and a pane MAY converge while the surface's handover is still refused. The
convergence of one pane MUST NOT be described as progress on the rows refusing the
others, and the change that performs it MUST state that every gate row keeps its verdict.

The visible consequence SHALL be recorded as a decided behavioural change with its reason
— which rows are beside the cursor when the list overflows — rather than left to be
discovered in a frame diff.

#### Scenario: The window converges first

- **WHEN** a native pane whose window is its widget's is prepared for handover
- **THEN** the pane is changed to window by the kernel's rule in its own change, and the
  handover changes no window

#### Scenario: The pane's own arithmetic is a window like any other

- **WHEN** a native pane resolves its visible slice with inline arithmetic rather than
  through a widget, and folding that one call into the handover is proposed because it is
  small
- **THEN** it is refused, and the pane converges in a change of its own

#### Scenario: One pane of a multi-pane surface converges alone

- **WHEN** a surface drawn as two panes has one pane's window converged while its handover
  is still refused
- **THEN** the change states that it hands nothing over and that every gate row keeps its
  verdict

#### Scenario: The changed scrolling is stated

- **WHEN** convergence changes which rows sit beside the cursor in an overflowing list
- **THEN** the change states that consequence and why the kernel's rule is the one both
  panes take, rather than leaving it to a diff

#### Scenario: Converging the rule to the pane is refused

- **WHEN** closing the difference by changing the kernel's helper to the pane's widget
  behaviour is proposed
- **THEN** it is refused, because the helper is shared and the pane is the thing being
  deleted
