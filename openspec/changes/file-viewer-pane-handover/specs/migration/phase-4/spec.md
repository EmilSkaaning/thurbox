# migration/phase-4 Specification

## ADDED Requirements

### Requirement: The file viewer is drawn by its bundled plugin, and its renderer is deleted

The file viewer SHALL be handed over to its bundled plugin: `src/ui/file_viewer.rs` is
deleted, and the pane a user opens is the plugin's, drawn from the seat the native pane
occupied, gated by the flag that always gated it and bound to the key that always showed
it.

The handover SHALL grant the plugin **nothing**. The pane's seven scoped actions —
including the two whose effects are a **directory read** and a **process launch** —
remain the kernel's, resolved because the pane declares that it is thurbox's file viewer
and performed against the kernel's own state. The `files` capability MUST still publish a
basename per row and nothing else: no path, no contents, no directory listing, and no
search query.

The pane's search bar SHALL remain the kernel's, drawn as seat chrome in the rows it
always occupied.

The tree the pane shows MUST go on being rebuilt when the active session changes, and
MUST NOT be rebuilt while the pane is off screen — the native pane read no directory with
its column closed, and neither may the seat.

#### Scenario: The pane a user opens is the plugin's

- **WHEN** the key that shows the file viewer is pressed
- **THEN** the column appears in the position it always had, showing the plugin's tree,
  focused, and the key that hid it hides it

#### Scenario: The keys still do what they did

- **WHEN** the pane holds focus and its scoped keys are pressed
- **THEN** the cursor moves, a directory expands and collapses, a file opens in the
  configured editor, and the search runs — all performed by the kernel, all still
  rebindable

#### Scenario: No capability was widened to close the handover

- **WHEN** the plugin's manifest and the host's bindings are inspected after the handover
- **THEN** the manifest holds `render` and `files`, no binding reads a directory or a
  file, no binding launches a process, and the published row carries no path

#### Scenario: A hidden pane reads no directory

- **WHEN** the pane is not on screen and the active session changes
- **THEN** no directory is read for it

### Requirement: A row another change would flip is re-verdicted by that change

Where a change makes a **different** pane's recorded gate row probe differently, that
change SHALL re-verdict the row in the same commit: rewriting what the row stands on, and
its probe, to the fact that now decides it.

A row MUST NOT be left to flip on its own. A gate row's probe is a proxy for its reason,
and a proxy that stops matching its reason reports the opposite of the truth — "the seat
now exists, so the pane is unblocked" where the seat existing is precisely what makes the
pane's problem sharper.

Where the re-verdict makes the row's reason stronger rather than weaker, the change SHALL
say so, so that a reader of the two commits can see the verdict moved for a reason rather
than to keep a suite green.

#### Scenario: A seat another pane's gate asserts is absent is added

- **WHEN** a handover names a seat that another pane's gate row probed for the absence of
- **THEN** that row is rewritten in the same change to stand on what now blocks it, and
  the pane's verdict is unchanged unless its reason genuinely went

#### Scenario: The re-verdict is a strengthening

- **WHEN** the new fact makes the other pane's handover harder rather than easier
- **THEN** the change records that, rather than presenting the rewrite as bookkeeping
