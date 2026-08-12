# Design — the changed-files list's window and model

## What the pane derived from its own arithmetic

Three things, and the reason they are listed separately is the reason the session list's
were: a reader who sees only "which rows are on screen" concludes the gap is a wiring
detail.

| Derived from the inline `start`/`end` | Derived from the shared window |
|---|---|
| which tree rows are painted | the painter's own slice of the list's children |
| the click hitbox of each visible file | the row rects `render_tree_rows` returns |
| which row carries the selection appearance | the row's own runs, `TextStyle::selected` |

The third is the one that changed shape rather than owner. The native row decided its
appearance by comparing `current_file()` to the row's index *while painting the window*;
the tree decides it while **building** the row, and the painter never asks. That is what
lets a second drawer of this list — a plugin, which is never told the window — mark the
same row.

## Why the pane converges and not the rule

`migration/phase-4` states the constraint for the session list and it holds verbatim here:
the kernel's windowing rule "MUST NOT be closed by redefining the kernel's own windowing
rule to match one pane's" behaviour, because that rule is what every plugin list and four
native surfaces scroll by. The pane is the thing scheduled for deletion, so the pane moves.

### Rejected: leave the window alone and converge it inside the handover

The tempting order, because this pane's convergence is small — one call, no state to
delete. It is refused by `migration/handover`'s window rule for a reason that does not
scale with size: a handover claims that *which code draws a pane* changed and nothing else
did, and a commit that also changes which rows sit beside the cursor makes that claim
unverifiable. The recording taken at handover time would then move for two reasons at
once, and the recording is the only evidence that survives the deletion.

### Rejected: teach the shared rule this pane's cursor-at-the-bottom behaviour

The same move ADR-63 refused, on the same grounds, and one further one here: the two rules
disagree in *opposite* directions from the widget case. The session list's widget held its
offset until the cursor left the viewport; this pane recomputes its window every frame and
pins the cursor to the bottom row. A helper taught both would have three behaviours
selected by a flag, which is not a shared rule.

### Rejected: fold a folder header into the file below it

The session list folds a repo-group header into the session it heads, so that one index
names the same row in both panes. This tree deliberately does not: a folder header
precedes *several* files, so folding would either duplicate it or give the first file a
two-line row the others do not have — and a click on a folder would then select a file,
where today it selects nothing. Keeping one list child per tree row preserves the native
pane's own numbering, which is what the hitbox filter below relies on.

## Where the model goes

`migration/handover` decides this rather than the author: a model that performs **no**
effects and is a pure function of data the pure-data layer already owns is relocated into
that layer. `build_file_tree` takes `&[DiffFile]` — a `session::review` type — sorts, folds
and returns rows; it reads nothing and launches nothing. So it goes to `session::review`,
where `pair_hunk` already sits for the same reason (both the row builder in `app` and the
renderer in `ui` need the identical answer, and a second implementation diverges).

`FileTreeRow` carries an **index** rather than a `DiffFile` reference, unchanged from
`TreeRow`. That is what lets the rows cross to a reader that has no `DiffFile` at all — a
publication — without the type growing a lifetime.

The architecture allowlist is unaffected: `session::review` gains no reference (it already
holds only `super` types), and `ui::code_review` keeps the `crate::session::review` import
it already had.

## The hitbox filter

`render_tree_rows` reports one hitbox per row of the outermost list, numbered from one in
list space. This list's children are tree rows, so index `n - 1` is a `FileTreeRow`: a
`File` row maps to its `index` (the file's position in the review's own `files`, preserved
across the sort) and a `Folder` row maps to nothing. That is the native behaviour — a
directory header was never a hitbox — expressed as a filter over the paint rather than as
an accumulator inside it, which is what makes the numbering the same in both panes.

## What the tests are evidence of

- `the_changed_files_tree_paints_what_the_span_builder_painted` — the pre-port span
  builders are kept in the test module and the two paints are compared as buffers, which
  covers colour and modifier as well as glyph. Two widths: one that fits, and one narrow
  enough that a folder name is cut, so the tree's `ellipsize` is asserted to land where
  `truncate` landed.
- `the_changed_files_window_is_the_shared_rule` — the stated behaviour change, pinned
  positively: with twelve files in five content rows and the cursor on the tenth, that file
  is third of five and the window is clamped at the tail.
- `file_tree_groups_dirs_and_keeps_indices` moves with the model it tests, to
  `src/session/review.rs`.
