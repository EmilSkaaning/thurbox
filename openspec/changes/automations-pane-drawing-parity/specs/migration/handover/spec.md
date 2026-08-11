# migration/handover Specification

## ADDED Requirements

### Requirement: A pane's frame is converged before its handover, not during it

The host draws one frame around every plugin pane, whatever seat it holds: a seat
decides *where* a pane is drawn and never *how*. So a native pane whose own frame
differs from that one — different border corners, a different focused colour, a
different title style, or a focus level it collapses into another — SHALL be converged
onto the host's frame in a change **before** its handover, and MUST NOT be converged
inside it.

The reason is what a handover is allowed to claim. A handover asserts that which code
draws a pane changed and nothing else about the pane did; a commit that also restyles
the border makes that claim unverifiable, because a reviewer cannot separate the
intended restyle from a regression and the frame snapshot moves for two reasons at
once.

Convergence MUST run in the direction of the host's frame, never the other way: the
manifest MUST NOT grow border or title options so a pane can bring its own frame. A
plugin-declared frame would let a pane draw itself as focused when it is not, which is
the confusion resolving the level from the kernel's own focus exists to prevent.

Where convergence starts drawing a focus level the native pane was collapsing, that
consequence SHALL be recorded as a visible change with its reason, rather than left to
be discovered in a frame diff.

#### Scenario: The frame converges first

- **WHEN** a native pane whose frame differs from the host's is prepared for handover
- **THEN** the native pane is changed to draw the host's frame in its own change, and
  the handover changes no border

#### Scenario: A plugin cannot bring its own frame

- **WHEN** closing the difference from the plugin's side is proposed
- **THEN** it is refused, because the frame is the host's and a pane's focus is the
  kernel's to resolve

#### Scenario: A collapsed focus level starts being drawn

- **WHEN** the host's frame distinguishes a level the native pane drew as another
- **THEN** the change records that the pane now draws it, and why that reading is the
  correct one
