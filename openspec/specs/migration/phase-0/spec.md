# migration/phase-0 Specification

## Purpose
Defines what Phase 0 of the v2 migration must demonstrate before the pane
migration proper begins: that the view tree can carry a real v1 pane without
changing what that pane draws, and that whatever the catalogue could not express
is reported rather than worked around. Phase 0 proves the contract Phase 4 will
be written against — so a gap quietly patched in a pane, instead of closed in the
catalogue, defeats the phase's whole purpose.
## Requirements
### Requirement: A real v1 pane renders through the view tree

At least one of thurbox's own panes SHALL build a `session::view_tree::ViewNode`
and be painted by the view-tree renderer, rather than by hand-built terminal
primitives. The pane SHALL remain in-process Rust with no plugin VM involved —
the point is to establish that the catalogue can carry a real pane, not to ship
a plugin.

The pane originally chosen was the **info panel**, because it is the pane with the
most kernel state and the most geometry per row, and so exercised the catalogue
hardest. That pane has since been **handed over** and its renderer deleted, so the
requirement is carried by the native panes that remain — the **tasks pane** is the
one named here, since it is the smallest of them and its port established the
scroll-window rule the others reuse.

Naming a successor rather than dropping the requirement is deliberate: what Phase 0
established is that the tree and its renderer are *kernel* code, independent of the
plugin host. That is still true, and it is why the handed-over pane's replacement
paints through the same renderer.

#### Scenario: The pane builds a view tree

- **WHEN** a native pane that has not been handed over is rendered
- **THEN** it constructs a view-tree node and paints it through the shared
  view-tree renderer, and no ratatui line is assembled outside that renderer

#### Scenario: No plugin runtime is involved

- **WHEN** thurbox is built without the plugin feature
- **THEN** the still-native panes render through the view tree, since the tree and
  its renderer are kernel code and not gated on the plugin host

#### Scenario: A handed-over pane's renderer is not a counter-example

- **WHEN** the requirement is checked after a pane has been handed over
- **THEN** that pane's absence does not violate it, because the requirement asks
  for at least one native pane on the tree and names one that exists

### Requirement: The ported pane is byte-identical to the pane it replaces

The port SHALL NOT change what the pane draws. A pinned frame of the pane with
every optional row populated MUST be recorded from the pre-port renderer and
MUST NOT move when the port lands. Any divergence that cannot be avoided MUST be
enumerated, justified, and pinned by its own test — never absorbed by updating
the pinned frame.

The pre-port line builders retained as that oracle SHALL live for as long as the
pane does. A **handover** deletes the pane, and with it both sides of this
comparison: there is no pre-port renderer to compare against and no native pane to
compare. What outlives the pane is the recorded view-tree expectation the handover's
own evidence rule requires, so this requirement binds a pane that thurbox still
draws and is discharged — not weakened — by that pane's deletion.

#### Scenario: The pinned frame does not move

- **WHEN** a pane thurbox still draws natively is rendered after its port at the
  size the frame was recorded at
- **THEN** the frame matches the recording character for character

#### Scenario: Styling is identical, not merely similar

- **WHEN** the ported pane and the pre-port line builders are each rendered into
  a buffer across a range of widths and content variants
- **THEN** every cell agrees in symbol, foreground colour and modifiers, save
  for the enumerated divergences

#### Scenario: A divergence is pinned rather than hidden

- **WHEN** the port changes what the pane draws for some input
- **THEN** a test asserts the new behaviour and names why it is preferred, and
  the pinned frame is unchanged

#### Scenario: The pane is handed over

- **WHEN** a pane's native renderer is deleted because a plugin draws it instead
- **THEN** the pre-port oracle goes with it, and the pane's continuing evidence is
  the recorded view tree rather than a pinned frame of a renderer that no longer
  exists

### Requirement: What the port could not express is reported

Porting the pane SHALL report every catalogue gap it hit, and each gap MUST
either be closed in the same change or recorded as still open with the reason.
The value of the exercise is the audit, so a gap that was worked around in the
pane rather than closed in the catalogue MUST be recorded as still open.

#### Scenario: A gap closed by widening the catalogue

- **WHEN** the pane needed something the catalogue could not express
- **THEN** the catalogue was widened by an accompanying specification change,
  and the pane-readiness audit records the gap as closed with the commit that
  closed it

#### Scenario: A gap left open

- **WHEN** the pane depends on something a plugin still could not obtain
- **THEN** the pane-readiness audit records that gap as still open, so the next
  pane's port does not assume it was settled

