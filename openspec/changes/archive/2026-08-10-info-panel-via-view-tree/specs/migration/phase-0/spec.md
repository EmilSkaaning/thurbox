# migration/phase-0 Specification

## ADDED Requirements

### Requirement: A real v1 pane renders through the view tree

At least one of thurbox's own panes SHALL build a `session::view_tree::ViewNode`
and be painted by the view-tree renderer, rather than by hand-built terminal
primitives. The pane SHALL remain in-process Rust with no plugin VM involved —
the point is to establish that the catalogue can carry a real pane, not to ship
a plugin.

The chosen pane is the **info panel**, because it is the pane with the most
kernel state and the most geometry per row, and so exercises the catalogue
hardest.

#### Scenario: The pane builds a view tree

- **WHEN** the info panel is rendered
- **THEN** it constructs a view-tree node and paints it through the shared
  view-tree renderer, and no ratatui line is assembled outside that renderer

#### Scenario: No plugin runtime is involved

- **WHEN** thurbox is built without the plugin feature
- **THEN** the info panel still renders through the view tree, since the tree
  and its renderer are kernel code and not gated on the plugin host

### Requirement: The ported pane is byte-identical to the pane it replaces

The port SHALL NOT change what the pane draws. A pinned frame of the pane with
every optional row populated MUST be recorded from the pre-port renderer and
MUST NOT move when the port lands. Any divergence that cannot be avoided MUST be
enumerated, justified, and pinned by its own test — never absorbed by updating
the pinned frame.

#### Scenario: The pinned frame does not move

- **WHEN** the pane is rendered after the port at the size the frame was
  recorded at
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
