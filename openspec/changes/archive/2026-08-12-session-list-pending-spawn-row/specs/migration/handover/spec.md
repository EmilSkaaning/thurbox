# migration/handover delta

## MODIFIED Requirements

### Requirement: A handover relocates the model the deleted module also held

Where the module a handover deletes holds more than the pane's renderer — the pane's
**model**, or a helper other surfaces share — each part SHALL be relocated to the layer
that owns it, and the relocation MUST NOT change behaviour.

The relocation SHALL happen in the handover's own change, **except** where the handover is
refused on a requirement the relocation does not close. In that case the relocation SHALL
be hoisted into a change of its own, ahead of the handover, for the reason a pane's
keyboard is hoisted: a change that both relocates a model and moves who draws a pane makes
any behavioural difference read equally as either, and a model stranded in a rendering
layer by an unrelated refusal is stranded for as long as that refusal stands. A hoisted
relocation MUST state that it hands no pane over, and the pane's gate row for the
relocation MUST be re-verdicted by it while every other row keeps its verdict.

The pane's model belongs to whatever already owns its value. A model that performs
side effects — reading directories, launching a process — MUST NOT be relocated into a
layer the architecture keeps free of them, however well its *types* would fit there: a
pure-data layer holding I/O is a worse outcome than a coordinator holding a state
machine. Correspondingly, a model that performs **no** effects and is a pure function of
data that layer already owns SHALL be relocated into it, rather than into the coordinator,
so that the two cases are decided by the same rule rather than by which came first.

A part of the model that is **downstream of a seam the refusal has not settled** SHALL NOT
be relocated with the rest, and the exclusion MUST be stated. Relocating it would fix its
home against a seam that does not exist yet. Once that seam **is** settled, the excluded
part SHALL be relocated by the same rule, in the change that settles the row it was
downstream of — not deferred to the handover, which would leave the handover holding the
relocation the hoist exists to keep out of it.

Geometry SHALL NOT cross into the pure-data layer with the model. Where the model and a
width-dependent fit share a type, the type MAY move while the fit stays, provided the
moved producer leaves the geometry-bearing field unset.

A shared helper MUST move to the layer's own shared vocabulary rather than to one of its
callers, so that no surface windows a list by a rule that lives in a different pane's
module.

Where the relocation leaves two types with the same fields and one producer, the
duplicate SHALL be **deleted** rather than carried: a handover is the moment the second
one stops having a reason to exist.

A relocation MUST NOT be satisfied by re-exporting the moved items from the module they
left. The re-export preserves exactly what the relocation exists to remove — the module
remains the name the kernel calls its own model by, and the handover's deletion problem is
unchanged.

#### Scenario: The deleted module was also the model

- **WHEN** a pane's renderer is deleted and its state machine lived in the same module
- **THEN** the state machine is relocated to the layer that owns the value, unchanged,
  and the pane behaves identically

#### Scenario: The model performs side effects

- **WHEN** relocating that state machine into the pure-data layer is proposed because its
  types would fit
- **THEN** it is refused, because the model reads the filesystem and that layer is kept
  free of effects

#### Scenario: The model performs no side effects

- **WHEN** the model is a pure function of data the pure-data layer already owns
- **THEN** it is relocated into that layer rather than into the coordinator, by the same
  rule that refused the previous case

#### Scenario: The handover is refused on an unrelated requirement

- **WHEN** a pane's model sits in its renderer and the pane's handover is blocked by a
  requirement the relocation does not close
- **THEN** the relocation is done in its own change ahead of the handover, that change
  hands no pane over, and only the relocation's own gate row is re-verdicted

#### Scenario: Part of the model is downstream of an unsettled seam

- **WHEN** one function of the model is consumed by the very seam the refusal is about
- **THEN** it stays where it is, and the change states why it was excluded

#### Scenario: The seam the excluded part waited on is settled

- **WHEN** the row that part was downstream of closes
- **THEN** it is relocated in that same change, rather than being carried into the
  handover

#### Scenario: The relocation is proposed as a re-export

- **WHEN** the moved items are re-exported from their old module so no caller changes
- **THEN** it is refused, because the kernel would still name its own model through the
  rendering layer and the module would still be undeletable

#### Scenario: A shared helper outlives the module

- **WHEN** the deleted module held a helper other surfaces call
- **THEN** the helper moves to the layer's shared vocabulary and every caller is updated
  in the same change

#### Scenario: The relocation exposes a duplicate type

- **WHEN** the relocated model's row type has the same fields as the published row type
  and the publication is now its only consumer
- **THEN** one of the two is deleted rather than both being kept
