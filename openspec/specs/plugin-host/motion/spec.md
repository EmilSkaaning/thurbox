# plugin-host/motion Specification

## Purpose
TBD - created by archiving change plugin-motion-leases. Update Purpose after archive.
## Requirements
### Requirement: Motion is declared, never pushed

A view node SHALL be able to declare motion — a change over time the host
evaluates on its own clock from content the plugin supplied in the pushed tree.
The host MUST NOT provide any means for a plugin to cause, schedule, or
otherwise request a frame, and evaluating a motion MUST NOT call into the
plugin.

#### Scenario: A plugin declares motion and pushes once

- **WHEN** a plugin pushes a tree containing a motion declaration and pushes
  nothing further
- **THEN** the pane's rendered content advances over time without any further
  render call into the plugin

#### Scenario: The plugin is not consulted per frame

- **WHEN** the host advances a motion between two paints
- **THEN** no plugin code runs as part of that advance

### Requirement: The motion catalogue is closed and named

The host SHALL define a closed set of motion kinds and MUST reject a
declaration naming any other kind, identifying the kinds it does define. A
declaration MUST carry between two and sixty-four frames; a declaration outside
that range MUST be rejected rather than clamped, so a plugin learns its
animation was malformed instead of silently getting a different one.

#### Scenario: An unknown kind

- **WHEN** a plugin declares a motion whose kind the host does not define
- **THEN** conversion fails, naming both the unknown kind and the kinds the
  host defines

#### Scenario: Too few frames

- **WHEN** a plugin declares a motion with fewer than two frames
- **THEN** conversion fails rather than rendering a static node

#### Scenario: Too many frames

- **WHEN** a plugin declares a motion with more frames than the host allows
- **THEN** conversion fails rather than truncating the frame list

#### Scenario: Frames count against the tree budget

- **WHEN** a motion's frames push the tree past the host's node bound
- **THEN** the tree is rejected exactly as any other oversized tree is

### Requirement: A declared rate is clamped to the per-pane cap

The host SHALL clamp a declared frame rate into a bounded range and MUST NOT
serve a pane faster than the per-pane cap, whatever the plugin declared. An
omitted rate MUST take the host's default.

#### Scenario: A plugin declares an absurd rate

- **WHEN** a plugin declares a rate far above the per-pane cap
- **THEN** the motion is served at the cap

#### Scenario: A plugin declares a rate below the floor

- **WHEN** a plugin declares a rate of zero or a negative rate
- **THEN** the motion is served at the host's minimum rate rather than never
  advancing or dividing by zero

### Requirement: Motion identity preserves phase across re-pushes

Motion state SHALL be keyed by the pane, the node's identity, and a signature
of the motion's kind and parameters, and each key MUST carry the clock time it
was first seen. Re-pushing an identical motion on the same node identity MUST
continue the animation from its current frame. A new epoch MUST begin only when
the node identity changes, the signature changes, or the pane's motion state
was dropped.

#### Scenario: An identical re-push does not restart

- **WHEN** a plugin re-pushes a tree whose motion declaration and node identity
  are unchanged, after time has passed
- **THEN** the rendered frame is the one time has reached, not the first frame

#### Scenario: Changing the declaration restarts

- **WHEN** a plugin pushes a motion on the same node whose parameters differ
  from the previous push
- **THEN** the animation restarts from its first frame

#### Scenario: Changing the node identity restarts

- **WHEN** a plugin pushes the same motion declaration under a different node
  identity
- **THEN** that node's animation starts from its first frame

#### Scenario: An identity-less node is keyed structurally

- **WHEN** a plugin declares motion on a node that carries no identity
- **THEN** the host keys it by its position in the tree and records that it did
  so, so the animation still runs and the cause of a later restart is
  diagnosable

### Requirement: A lease is granted only while live motion is on screen

A pane whose visible tree contains live motion SHALL hold an animation lease
that exempts that pane — and only that pane — from the host's forced-redraw
floor, up to its served rate. The lease MUST be dropped when the pane is
hidden, when its next tree contains no motion, when a non-repeating motion
reaches its last frame, and when the pane ceases to exist.

#### Scenario: A visible animated pane holds a lease

- **WHEN** a visible pane's tree contains live motion
- **THEN** the pane holds exactly one lease regardless of how many animated
  nodes its tree contains

#### Scenario: Hiding the pane drops the lease

- **WHEN** a pane holding a lease is hidden
- **THEN** the lease is dropped and the host stops repainting on its account

#### Scenario: A tree without motion drops the lease

- **WHEN** a plugin pushes a tree with no motion in it
- **THEN** the pane's lease is dropped

#### Scenario: A finished non-repeating motion drops the lease

- **WHEN** a motion declared not to repeat reaches its last frame
- **THEN** the lease is dropped and the last frame stays on screen

### Requirement: Motion state cannot outlive what declared it

The host SHALL discard the recorded state for any motion key that is not
present in the current trees. Motion state MUST NOT accumulate across pushes,
pane hides, or pane removals.

#### Scenario: A removed node's state is discarded

- **WHEN** a plugin pushes a tree that no longer contains a previously animated
  node
- **THEN** the host holds no motion state for that node

#### Scenario: Re-showing a hidden pane starts a new epoch

- **WHEN** a pane holding motion is hidden and later shown again
- **THEN** its motion begins from its first frame rather than resuming a phase
  that ran while nothing was drawn

#### Scenario: Repeated pushes do not grow state

- **WHEN** a plugin pushes the same animated tree many times
- **THEN** the host's motion state holds one entry per animated node, not one
  per push

### Requirement: Aggregate rate is bounded and degrades by freezing

The host SHALL bound the total frame rate across all leases. When declared
rates exceed that bound, the focused pane MUST keep its declared rate up to the
per-pane cap, the remainder MUST be distributed over the other leases in
ascending declared rate, and a lease that cannot be served at the host's
minimum readable rate MUST be frozen at its current frame rather than served at
a rate that reads as stutter.

#### Scenario: Leases within budget are served in full

- **WHEN** the sum of declared rates is within the aggregate bound
- **THEN** every lease is served at its declared rate

#### Scenario: The focused pane is served first

- **WHEN** declared rates exceed the aggregate bound
- **THEN** the focused pane is served at its declared rate, up to the per-pane
  cap

#### Scenario: A starved lease is frozen, not stuttered

- **WHEN** the remaining budget cannot serve a lease at the minimum readable
  rate
- **THEN** that lease is frozen at its current frame and counted, rather than
  every lease being slowed proportionally

### Requirement: Reduced motion suppresses every animation

The host SHALL provide a single application-wide reduced-motion setting. With
it on, no lease MUST be granted, every motion MUST render its first frame, and
the host's own animated indicators MUST stop animating too.

#### Scenario: Plugin motion under reduced motion

- **WHEN** reduced motion is on and a pane's tree contains motion
- **THEN** the pane renders the motion's first frame, holds no lease, and does
  not repaint on the animation's account

#### Scenario: The host's own spinner under reduced motion

- **WHEN** reduced motion is on and a session is working
- **THEN** the host's status indicator stays on its first frame and does not
  request repaints to advance

#### Scenario: Reduced motion turned on while an indicator is animating

- **WHEN** reduced motion is turned on while the host's spinner is mid-cycle
- **THEN** the spinner settles on its first frame — the same glyph it would show
  had the setting been on since launch — and requests no repaints thereafter

#### Scenario: Reduced motion applies without a restart

- **WHEN** reduced motion is changed in `settings.toml` or the settings panel
- **THEN** the running host applies it immediately and does not report the
  change as requiring a restart

#### Scenario: Reduced motion is off by default

- **WHEN** no reduced-motion preference is configured
- **THEN** motion runs

### Requirement: Motion does not defeat the demand-driven render loop

The host SHALL mark the interface dirty on a motion's account only when the
frame that motion resolves to actually changes. A pane animating below the
loop's iteration rate MUST NOT cause a paint per iteration, and a pane with no
live lease MUST leave the idle paint rate exactly as it was without any
animation present.

#### Scenario: An animation repaints at its rate, not the loop's

- **WHEN** a visible pane animates at a rate far below the render loop's
  iteration rate
- **THEN** paints attributable to the animation occur at approximately the
  animation's rate

#### Scenario: A hidden animated pane costs nothing

- **WHEN** a pane containing motion is hidden
- **THEN** the idle paint rate is the same as with no animation declared at all

### Requirement: Motion is evaluated from the host clock

Motion SHALL be evaluated through the same clock the host's other time-driven
behaviour reads, so that a test can fast-forward it, and a rendering taken
without advancing that clock MUST show the first frame.

#### Scenario: A snapshot renders the first frame

- **WHEN** a frame is rendered without the clock having advanced past a
  motion's epoch
- **THEN** the motion's first frame is drawn

#### Scenario: Fast-forwarding advances the animation

- **WHEN** the host clock is advanced past one frame interval
- **THEN** the next rendering shows the next frame

### Requirement: Motion cost is counted

The host SHALL expose wall-clock-free counters for leases granted, frames
advanced, motions denied, and leases frozen, so that repaints attributable to
animation are attributable without timing measurement.

#### Scenario: Granting a lease is counted

- **WHEN** a pane's tree first contains live motion
- **THEN** the lease-granted counter increases

#### Scenario: Suppressed motion is counted as denied

- **WHEN** reduced motion suppresses a declared motion
- **THEN** the denied counter increases and the frames counter does not

