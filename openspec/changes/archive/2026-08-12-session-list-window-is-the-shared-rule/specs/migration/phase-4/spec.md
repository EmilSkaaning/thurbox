# migration/phase-4 delta

## MODIFIED Requirements

### Requirement: A pane whose window is its widget's is refused until the window is a seam

A native pane MAY resolve which of its rows are on screen through a list widget that
keeps its own scroll offset. Where **other behaviours are derived from that offset** —
clipped-row indicators, click hitboxes, the position of a row the kernel inserts — the
pane SHALL NOT be handed over until that window is a seam both occupants resolve
through.

The reason is not that the reproduction cannot scroll. It scrolls by the kernel's own
rule, from a declared cursor; the rule is simply a **different** one, over a different
row count where the native widget folds one row into another. So at any height where the
list overflows the two panes show different rows — which for a pane whose selection
drives what other panes display is a behavioural change, not a rendering divergence.

The refusal MUST enumerate each behaviour derived from the offset separately, because a
reader who sees only "which rows are on screen" concludes the gap is a wiring detail.

The window MUST NOT be closed by redefining the kernel's own windowing rule to match one
pane's widget: that rule is what every plugin list and several native panes scroll by, so
a change for one pane changes all of them.

The row SHALL be closed instead by converging the **pane** onto that rule — the native
pane resolving its window, its clipped-row indicators and its click hitboxes through the
same painter and the same windowing helper its reproduction goes through, and both trees
folding a header and the row it heads into one list item so one index names the same row
in both. The direction is the constraint: the shared rule is unchanged, and it is the
pane scheduled for deletion that moves.

The relocation of anything the widget's window feeds MUST be ordered **after** this
decision, since what a windowing seam looks like decides where those functions live.

#### Scenario: The two windows disagree

- **WHEN** a list longer than the pane is drawn by the native pane through a list widget
  and by its reproduction through the kernel's rule
- **THEN** both keep the cursor visible, the sets of other visible rows differ, and the
  gate records the difference as structural

#### Scenario: Redefining the shared rule is refused

- **WHEN** matching the widget's behaviour by changing the kernel's windowing helper is
  proposed
- **THEN** it is refused, because that helper is shared by every plugin list and several
  native panes

#### Scenario: The pane converges onto the shared rule

- **WHEN** the native pane is changed to window through the kernel's helper, to fold a
  header into the row it heads, and to read its indicators and hitboxes off the paint
- **THEN** the two panes draw the same rows at the same height, and the row is recorded
  closed with the behaviour it changed
