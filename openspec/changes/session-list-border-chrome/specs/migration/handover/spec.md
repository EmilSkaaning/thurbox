# migration/handover delta

## MODIFIED Requirements

### Requirement: Chrome a plugin cannot draw stays the kernel's, inside the seat

A handed-over pane MAY have chrome the plugin has no way to draw — a row of key hints
naming **rebindable kernel chords**, an input bar whose text is kernel state, a status
summary painted on the pane's own frame. That chrome SHALL keep being drawn by the
kernel, in the seat, in the position it had; the plugin's tree is laid out in what
remains, which is the same area the native pane laid its own content out in.

It MUST NOT be published to the plugin instead. A chord is a user's setting the kernel
resolves, and a pane redrawing it from published state would be a second renderer for
one fact — while a plugin *inventing* the hint would print a chord the user may have
rebound. The same holds for an input bar: the kernel owns the key that opens it, so it
owns the query, the caret and the match count, and publishing them to be redrawn would
add state to a capability that deliberately withholds it.

Chrome MUST NOT be restricted to a single row. Where the native pane drew a bordered,
multi-row band — outside its own frame rather than inside it — the kernel SHALL draw the
same band in the same place, subtracting it from the seat before the pane's frame is
drawn.

Chrome MAY be drawn **on the pane's frame** rather than in the seat's interior, where
that is where the native pane drew it. Such chrome SHALL subtract nothing: the pane's
content area, and therefore its row hitboxes, MUST be exactly what they would be without
it. A plugin MUST NOT be able to draw there, ask for it, or suppress it — the frame is
the host's.

Chrome SHALL be described to the seat as **data**, in a closed set of shapes, rather than
as a painter the seat invokes: a painter argument would make "the kernel draws whatever it
likes inside a plugin pane" the rule, where the point is that what a seat may draw stays
enumerable.

The chrome MUST appear under the same condition it appeared before the handover, and
the pane's content area MUST be the area it had, so a handover changes which code
draws the pane's content and nothing else about the pane. Different chrome MAY have
different conditions — a hint row that follows focus and an input bar that follows its
own sub-mode — and each MUST keep the condition its native counterpart had.

#### Scenario: The hint row survives the handover

- **WHEN** a handed-over pane whose native counterpart drew a key-hint row holds focus
- **THEN** the row is drawn in the same position, and the plugin's tree occupies the
  rest of the seat

#### Scenario: The chrome follows its own condition

- **WHEN** that pane does not hold focus
- **THEN** the row is not drawn and the plugin's tree occupies the whole seat

#### Scenario: The chrome is an input bar the native pane drew below its frame

- **WHEN** a handed-over pane's search sub-mode is active or its query is committed
- **THEN** the kernel draws the same bordered bar, in the same rows, with the query, the
  caret and the match count it always showed, and the pane's frame occupies the rest of
  the seat

#### Scenario: Publishing the bar's state instead is proposed

- **WHEN** it is proposed that the plugin draw the bar from published state
- **THEN** it is refused, because the query is the kernel's and the capability publishes
  no query

#### Scenario: The chrome is a status summary on the pane's border

- **WHEN** a pane declares it is thurbox's session list and the kernel has sessions to
  report
- **THEN** one status dot per session, in that session's status colour and animated on
  the kernel's own spinner frame, is drawn right-aligned in the pane's top border

#### Scenario: Border chrome costs the pane no content row

- **WHEN** the same pane's tree is painted with and without that chrome present
- **THEN** the tree occupies the identical area and reports the identical row hitboxes

#### Scenario: A pane with nothing to report gets no border chrome

- **WHEN** the kernel has no sessions to summarise
- **THEN** no dots are drawn, exactly as the native pane drew none
