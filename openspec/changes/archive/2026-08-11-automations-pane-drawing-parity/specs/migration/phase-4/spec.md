# migration/phase-4 Specification

## MODIFIED Requirements

### Requirement: A geometry-free row clips where the native row ellipsizes

A view tree carries no width. While the catalogue had no way to say otherwise, a pane
whose native renderer **truncates** a row to the pane's width was recorded as
diverging in that row's last columns, attributed to the same missing fact that blocks
the pane's width-dependent layouts.

Once the catalogue **can** say it — a run declaring that it yields its width, which
the kernel ellipsizes — a divergence is no longer an acceptable record for a pane that
only needed a fit. Such a pane SHALL close it, and closing it has two halves that MUST
land together:

- the reproduction declares the yielding run, and
- the **native** pane stops fitting the text itself.

Both, because a pane that keeps cutting the string in its own tree while its
reproduction declares the fit produces trees that differ **by construction** — the
native tree carries an already-cut name and the plugin's carries a whole one — so no
width makes the two equal and the equality test can only be kept passing by comparing
at a width where the fit is a no-op.

A pane MUST NOT close it by publishing a resolved width, and MUST NOT close it by
publishing an already-fitted string. A width is resolved during a frame while the
snapshot is published on the tick, and the two panes' rects are not the same rect
while both exist; a fitted string would make the snapshot carry the pane's rendering
rather than the model's fact.

Where the fitted text is split into several runs — a name segmented at a search's
matched offsets — the runs MUST be cut as one piece of text, so the pane and its
reproduction agree with the kernel's own fitting rather than each cutting per run.

A pane whose divergence is still recorded rather than closed MUST say which of the two
halves is missing, so "the vocabulary cannot express it" is never inferred from a
record that means "this pane has not adopted it".

#### Scenario: A row that fits is identical

- **WHEN** a truncating row kind is painted at a width its text fits in
- **THEN** the plugin's frame and the native frame are identical cell for cell

#### Scenario: A row that overflows is identical too

- **WHEN** the same row is painted at a width its text overflows, after both halves
  have landed
- **THEN** both frames end that run in an ellipsis at the same column, and every run
  after it is still drawn

#### Scenario: A row that overflows clips

- **WHEN** a pane has not adopted the declaration and one of its rows overflows
- **THEN** the plugin's row is clipped at the pane's edge, the native row ends in an
  ellipsis one column earlier, and the record names the **native pane's fitting** as
  what is outstanding rather than the vocabulary
