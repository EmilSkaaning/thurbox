# phase-4 (delta)

## MODIFIED Requirements

### Requirement: A plugin's view of kernel state trails the kernel's by the render interval

The host SHALL be honest about the latency between a change in kernel state and a
plugin pane redrawn from it, and SHALL keep that latency bounded by a **rate
ceiling** rather than by a fixed render cadence. A pane is rendered when a source it
reads moves; a change arriving after the ceiling's interval has elapsed is rendered
immediately, and one arriving inside it waits out the remainder. So the trailing
interval is at most that ceiling, and is usually zero.

Because the ceiling is not zero, a pane's **cursor** MUST remain kernel state. A
pane that owned its own cursor would put the ceiling's remainder between a keypress
and the highlight moving, which is a latency a user feels directly, whereas a
published cursor moves in the frame the key was handled and only the plugin's
redrawn copy trails.

The ceiling, its worst case, and the rate at which the state behind it actually
moves MUST be recorded with the change that sets them, and MUST NOT be hidden by
having the plugin drive its own repaint. A port that depends on a plugin pane
reflecting kernel state promptly MUST state which of the two it relies on: that the
trigger is the change, or that the ceiling is small.

#### Scenario: The cursor moves

- **WHEN** the user moves the session-list cursor
- **THEN** the native pane's highlight moves on the next frame, and the plugin's
  reproduction of it is re-rendered because the session source moved — immediately
  unless a render pass happened within the ceiling's interval

#### Scenario: The interval is recorded

- **WHEN** a port depends on a plugin pane reflecting kernel state promptly
- **THEN** the audit records the rate ceiling, its worst-case delay, why the
  user-visible cursor is unaffected by it, and what closing the remainder would cost

#### Scenario: Nothing the pane reads has moved

- **WHEN** kernel state a pane does not read changes, or nothing changes at all
- **THEN** the pane is not re-rendered, so an idle interface enters no plugin VM
