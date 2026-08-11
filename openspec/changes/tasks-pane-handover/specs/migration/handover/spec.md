# migration/handover Specification

## ADDED Requirements

### Requirement: Chrome a plugin cannot draw stays the kernel's, inside the seat

A handed-over pane MAY have chrome the plugin has no way to draw — a row of key hints
naming **rebindable kernel chords**, an input bar whose text is kernel state. That
chrome SHALL keep being drawn by the kernel, in the seat, in the position it had; the
plugin's tree is laid out in what remains, which is the same area the native pane laid
its own content out in.

It MUST NOT be published to the plugin instead. A chord is a user's setting the kernel
resolves, and a pane redrawing it from published state would be a second renderer for
one fact — while a plugin *inventing* the hint would print a chord the user may have
rebound.

The chrome MUST appear under the same condition it appeared before the handover, and
the pane's content area MUST be the area it had, so a handover changes which code
draws the pane's content and nothing else about the pane.

#### Scenario: The hint row survives the handover

- **WHEN** a handed-over pane whose native counterpart drew a key-hint row holds focus
- **THEN** the row is drawn in the same position, and the plugin's tree occupies the
  rest of the seat

#### Scenario: The chrome follows its own condition

- **WHEN** that pane does not hold focus
- **THEN** the row is not drawn and the plugin's tree occupies the whole seat
