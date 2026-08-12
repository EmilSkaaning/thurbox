# migration/phase-4 delta

## ADDED Requirements

### Requirement: The review's changed-files list is reproduced by a bundled plugin pane

The code review's **changed-files list** SHALL be reproduced by a pane of the same
bundled plugin that reproduces its diff, so that both halves of a two-pane surface
have a reproduction before either is handed over.

The reproduction MUST be a second pane of the **same** plugin rather than a plugin of
its own: the two panes read one published section, are one surface to a user, and a
second manifest would be two lifecycles for one review.

It SHALL require **no capability the diff does not already hold**. Where a
reproduction needs a fact the publication lacks, that fact is added to the section
the surface already reads — a section grows, a grant does not.

The reproduction MUST NOT claim the seat its handover is refused on, and MUST NOT
declare the kernel keyboard of the pane it reproduces, while the native pane is what
the interface draws. Claiming the seat would place a copy where the original is;
declaring the keyboard would take the keys off a list the user can see and give them
to one nobody can. Both prohibitions SHALL be asserted against the shipped manifest.

The reproduction SHALL hold a **recorded** expectation derived from the kernel's own
tree builder while that builder exists, and the recording MUST cover every row kind
and every status name the list draws — a reproduction whose recording omits a kind is
evidence about the kinds it happens to include.

Porting the second pane SHALL NOT re-verdict any row of the surface's handover gate.
A reproduction is not a replacement, and the seat contest that refuses this pane is
unaffected by a copy existing in another column.

#### Scenario: The second pane reproduces the list

- **WHEN** the published changed-files rows are handed to the bundled pane and to the
  kernel's own tree builder
- **THEN** the two view trees are equal, row for row, including the selection
  appearance on the file the diff's cursor is in

#### Scenario: The port asks for nothing

- **WHEN** the reproduction's manifest is read
- **THEN** its capability list is the one the diff already declared, and it names
  neither the file-viewer seat nor a key context

#### Scenario: The gate is unmoved

- **WHEN** the port lands
- **THEN** the code review's handover gate records the same outstanding rows, and the
  change states that it hands nothing over
