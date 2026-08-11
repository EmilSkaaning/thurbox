# Phase 4 — bundled panes

## MODIFIED Requirements

### Requirement: A pane may be ported in part when its whole is not expressible

When a native pane is too large or too geometry-dependent to reproduce whole, the
port SHALL reproduce a named **core** of it completely, and SHALL itemise
everything left out in its proposal — one entry per omitted behaviour, each with
the reason it could not be drawn.

A partial port MUST NOT approximate what it omits. Drawing a diff without its row
tint, or a header without its rule, would make the reproduction agree with nothing
and the record a claim about a pane that does not exist.

The itemised remainder MUST be re-examined when the port is revisited, and each
entry MUST be classified as either **document** or **behaviour**: an entry is
document when the pane could draw it from published facts alone, and behaviour when
it needs a host power the plugin surface does not have. The classification is what
makes a partial port's remainder actionable rather than a standing list — the
document half is closable by publishing what the kernel already knows, and only the
behaviour half is a decision about the plugin surface.

The chosen core is the code-review view's **document**: every row kind the native
pane lists — file headers with their rule, fold chevron, status glyph and reviewed
mark, hunk headers with their `@@` ranges and reviewed mark, the unified diff lines
with their line-number gutter, syntax-coloured body and insertion/deletion row
tints, comments with their classification badges, the review summary's header and
its comments, informational rows — with the cursor's row drawn in the pane's
selection appearance.

#### Scenario: The core is reproduced completely

- **WHEN** the native renderer and the plugin are given the same review
- **THEN** the two paint the same row for every row kind the pane lists, across
  additions, deletions, context, the cursor's row, an empty body, each colour the
  highlighter assigns, a folded file, a reviewed file, a reviewed hunk, each file
  status, and each comment classification

#### Scenario: The remainder is a list, not a gap

- **WHEN** the port's proposal is read
- **THEN** every unported behaviour of the pane is named with the reason it is
  unported, classified as document or behaviour, and the readiness document carries
  the same list

#### Scenario: The document half is closed when it is closable

- **WHEN** a remainder entry is classified document
- **THEN** it is closed by publishing the facts the row is drawn from, rather than
  left on the list

## ADDED Requirements

### Requirement: A geometry-free row clips where the native row ellipsizes

A view tree carries no width, so a pane whose native renderer **truncates** a row to
the pane's width SHALL be recorded as diverging in that row's last columns: the tree
carries the whole text and the kernel's renderer clips it, where the native row cuts
one column earlier and writes an ellipsis.

The divergence MUST be enumerated with the row kinds it affects, and it MUST be
attributed to the **same** missing fact that blocks the pane's width-dependent
layouts rather than recorded as a separate gap. A port that split one missing fact
into several entries would overstate how much is left.

A port MUST NOT close this divergence by publishing a width. A resolved width in the
snapshot would make every published pane a geometry problem, and the pane that needs
it needs it for wrapping and pairing rather than for an ellipsis.

#### Scenario: A row that fits is identical

- **WHEN** a truncating row kind is painted at a width its text fits in
- **THEN** the plugin's frame and the native frame are identical cell for cell

#### Scenario: A row that overflows clips

- **WHEN** the same row is painted at a width its text overflows
- **THEN** the plugin's row is clipped at the edge, the native row ends in an
  ellipsis one column earlier, and the difference is the one the record names

### Requirement: A style token is added when the palette field it names has no token

When a ported pane draws in a palette field the token vocabulary does not name, the
port SHALL add a token for that field rather than reuse a token resolving a
different field.

A near-miss token is worse than a missing one: it paints a plausible colour, so the
equality test passes on the default theme and the pane diverges only on a custom
theme that sets the two fields apart — which is the case the token vocabulary exists
to serve.

#### Scenario: A near-miss token is refused

- **WHEN** a pane needs the palette's diff colours for a file header's insertion and
  deletion counts, and the vocabulary's insertion token resolves a different field
- **THEN** tokens for the diff colours are added, and the near-miss token is left
  resolving what it already resolved
