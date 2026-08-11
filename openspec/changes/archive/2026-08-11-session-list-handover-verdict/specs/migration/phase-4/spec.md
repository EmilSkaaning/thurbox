# migration/phase-4 Specification

## ADDED Requirements

### Requirement: A divergence a port enumerates becomes a gate row when its pane is refused

A port MAY enumerate a divergence from the native pane in its own test file, asserting
the inequality so the divergence cannot close unnoticed. That is a **measurement** and
it SHALL be kept.

When that pane's handover is refused, every enumerated divergence SHALL **also** appear
as a row in the pane's handover gate, with its own probe re-derived from the source. A
divergence recorded only in a test's documentation is a verdict written in prose, and a
verdict written in prose is a fact about a build that expires without telling anyone —
the same rule that put the refusal in a gate rather than in a document, applied to a
`///` block.

The two MUST NOT be collapsed into one. They fail for different reasons: the port's
assertion fails when the divergence **closes**, forcing the port to be revisited; the
gate row fails when the *tree* stops matching the recorded verdict. Deleting either
leaves the other unable to fail for its own reason.

A gate that already carries some of a pane's enumerated divergences and not others MUST
be completed rather than left inconsistent, since the omission is otherwise read as a
judgement that the missing ones do not block.

#### Scenario: A refusal is recorded for a pane with enumerated divergences

- **WHEN** a pane's handover is refused and its port enumerates divergences
- **THEN** each divergence has a row in the pane's gate, tagged by kind, with a probe
  that re-derives it

#### Scenario: A divergence closes

- **WHEN** the pane and its reproduction stop diverging
- **THEN** the port's inequality assertion fails and the gate row's recorded verdict
  disagrees with the tree, so both ask to be revisited

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

The relocation of anything the widget's window feeds MUST be ordered **after** this
decision, since what a windowing seam looks like decides where those functions live.

#### Scenario: The two windows disagree

- **WHEN** a list longer than the pane is drawn by the native pane and by its
  reproduction
- **THEN** both keep the cursor visible, the sets of other visible rows differ, and the
  gate records the difference as structural

#### Scenario: Redefining the shared rule is refused

- **WHEN** matching the widget's behaviour by changing the kernel's windowing helper is
  proposed
- **THEN** it is refused, because that helper is shared by every plugin list and several
  native panes

## MODIFIED Requirements

### Requirement: A wrap between two panes stays kernel-owned

Where two native panes form one continuous list — a movement key at the edge of one
moving focus into the other — that wrap SHALL remain the kernel's when one of the
panes becomes a plugin. Moving focus is view state, and no capability writes it.

While the pane holds its **own** keys, the plugin's share of the wrap is to
**decline** the key at its edge, which is what a consumed/not-consumed answer is for.
The port MUST record that the kernel's share — resolving an unconsumed movement key into
a focus change — is not implemented, so the key visibly does nothing at that edge, and
MUST NOT substitute a behaviour the native pane does not have (such as wrapping the
plugin's own cursor) and present it as parity.

The port MUST record that a wrap is a claim about adjacency, so it becomes
expressible only when the plugin's pane can sit where the native pane sits.

A **handover** onto the kernel keyboard closes the wrap without implementing that
kernel share at all, and the handover MUST record why: the handed-over pane is focused as
the kernel's own pane of that name, so both ends of the wrap are kernel focuses whoever
draws either pane, and the existing handlers complete it unchanged. The wrap therefore
needs no owner assigned, and survives one handover, both, or neither.

The handover MUST change the wrap's **condition** from the target pane's feature flag to
"a pane provides that list". The flag was a proxy that held only while the kernel drew the
target pane unconditionally; kept, a movement key at the edge would move focus into a
pane that is not on screen.

The reproduction's own declining half MUST be removed by that handover, since on the
keyboard route the plugin is never asked.

Once one of the two panes is handed over, the wrap SHALL be recorded as **not** a blocker
for the other's handover, and that MUST be asserted rather than described — otherwise a
later reader re-derives it as one, since it was a row in the first pane's gate for as long
as that pane held its own keys. The assertion MUST name the two facts that make it a
non-issue: both ends are kernel focuses, and the condition is already the pane's presence.

#### Scenario: The plugin declines at its edge

- **WHEN** a pane holding its own keys has its cursor on its first row and the
  previous-row key arrives
- **THEN** the plugin reports the key unconsumed

#### Scenario: Nothing completes the wrap

- **WHEN** an unconsumed key falls through from a focused plugin pane holding its own
  keys
- **THEN** no kernel action resolves it into a focus change

#### Scenario: The handover completes the wrap through the kernel's focus

- **WHEN** the pane is handed over on the kernel-keyboard route and a movement key is
  pressed at the adjacent pane's edge
- **THEN** focus moves into the handed-over pane exactly as before, and the plugin's
  declining half is gone

#### Scenario: The wrap's condition is the pane, not the flag

- **WHEN** no pane provides the target list
- **THEN** the movement key does not move focus into it

#### Scenario: The second pane's refusal does not list the wrap

- **WHEN** the other pane of the pair has its handover refused
- **THEN** the wrap is not among the reasons, and a rule asserts that both ends are
  kernel focuses and that the condition is the pane's presence
