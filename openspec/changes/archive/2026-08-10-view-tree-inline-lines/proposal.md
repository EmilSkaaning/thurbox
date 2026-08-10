# Let a plugin draw a line of differently-styled runs

## Why

The session-list frame-budget spike (recorded as `docs/SPIKE-SESSION-LIST.md`
recorded in `docs/SPIKE-SESSION-LIST.md`) cleared every
protocol bar and then found a blocker that has nothing to do with cost: **the
node catalog cannot express a single line of text carrying more than one
style.**

A pane row like v1's — a coloured status dot, then a bold name, then muted
activity text — has two renderings available today and both are wrong:

- `ui.row({…})` splits its area into **equal shares**
  (`Constraint::Ratio(1, n)`), because a plugin cannot specify widths and the
  kernel deliberately refuses to arbitrate between competing requests. A
  four-cell row in 40 columns gives the one-character dot ten columns and
  truncates the name.
- One pre-composed `ui.text(…)` lays out correctly and carries exactly **one**
  style, collapsing the dot's colour, the name's weight and the activity's
  muting into a single colour.

This is not a session-list problem. v1's info panel is 20-odd `label: value`
lines where the label is muted and the value is not; the tasks pane is a glyph
plus a title plus a marker; every focusable list row in thurbox is a line of
mixed styling. So the gap blocks the *first* pane of Phase 4, not the last, and
the spike's recommendation names it as a precondition: add it "before Phase 4
starts".

The fix is additive and small, and it is the one shape the catalog is missing:
a line whose children are laid out at their **intrinsic** width rather than in
equal shares.

## What Changes

- **A new `line` node.** Children are packed left to right on one terminal row,
  each taking exactly the width its content needs, and the line is clipped at
  the pane edge. It is the inline counterpart of `row`, which stays as it is —
  `row` divides an area, `line` composes a sentence.
- **Only inlineable children are allowed.** `text`, `motion` and a nested `line`
  have an intrinsic width; a `column`, `list`, `divider` or `spacer` does not,
  and putting one in a line is a **named conversion error** rather than a
  silently dropped or mis-measured node. The rule is recursive: a `motion`
  inside a line must have inlineable frames.
- **An animated glyph can sit inside a line.** A motion reserves the width of
  its **widest** frame and each narrower frame is padded to it, so an animation
  cannot shove the rest of the line sideways as it runs — the same
  stability rule `height_of` already applies to a motion's height, for the same
  reason.
- **`ui.line(children)`** joins the constructor table, and the `.d.luau`
  declaration and the bundled `hello` example gain it, since the example is what
  a plugin author copies.

## Capabilities

### Modified Capabilities

- `plugin-host/view-tree`: adds the inline line node to the closed catalog, its
  intrinsic-width layout rule, the inlineable-child restriction, and the
  motion-width reservation that keeps a line stable while an animation runs.

## Non-goals

- **No width hints on `row`.** The other way to close this gap is to let a
  plugin declare `size`/`flex` on a row's children. That is a layout solver:
  the kernel would have to arbitrate over-subscription, define what a
  percentage is a percentage of, and decide truncation priority — and every
  future node kind would inherit those questions. A line needs none of it,
  because intrinsic width is not a request the kernel can be asked to
  compromise. `row` keeps its equal shares and its stated rationale.
- **No wrapping.** A line that overruns its pane is clipped, matching `text`,
  which does not wrap either. Wrapping changes a node's height as a function of
  width, and `height_of` takes no width — introducing that is a separate change
  with its own measurable consequences for the layout pass.
- **No alignment or spacer-within-a-line.** Right-aligning a trailing run needs
  the resolved rect, which conversion does not have. A plugin that wants
  columns today pads with spaces in its own text; a `flex` span is additive
  later if a real pane needs it.
- **No new bounds.** A line's runs are ordinary children and already count
  against `MAX_NODES` and `MAX_DEPTH`. A separate span cap would be a second
  number saying the same thing.
