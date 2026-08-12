# The code review's changed-files list windows by the kernel's rule

## Why

`tests/code_review_pane_handover_gap.rs` refuses the code review's handover on five rows,
and the first of them is about a pane that is not the diff:

> `no-second-seat-for-the-changed-files-list` — the review's **second** pane, the
> changed-files list in the file-viewer column, with its own focus, its own navigation
> keys and a diff that follows its selection.

Closing that row means a plugin drawing that list — and `migration/handover` states two
things a handover may not do on the way. It may not converge a **window** inside itself
("A pane's window is converged before its handover, not during it"), and it may not
relocate a **model** inside itself when the handover is refused on rows the relocation
does not close ("A handover relocates the model the deleted module also held"). This
change is both of those steps for that pane, and it hands nothing over.

The v1 behaviour being replaced is the pane's own windowing arithmetic.
`ui::code_review::render_files_list` computed its visible slice inline —
`start = anchor - (height - 1)`, clamped — which keeps the current file pinned to the
**last** visible row once the list overflows. Every other list thurbox draws, native or
plugin, scrolls by `ui::visible_item_window`, which opens `min(height / 4, 3)` rows above
the cursor. Two rules, and the second pane of the review had the one nothing else uses.

That is not the widget case the existing requirement was written around — this pane never
held a `ListState` — which is why the requirement is broadened here rather than merely
applied: what makes a window a convergence problem is that it is **not the kernel's**, not
that a ratatui widget owns it.

## What Changes

- **The changed-files list is painted through the shared painter.**
  `render_files_list` builds a `ViewNode::List` and hands it to
  `ui::plugin_pane::render_tree_rows` — the renderer every plugin pane and, since ADR-63,
  the session list go through. Its window is `ui::visible_item_window`, and its click
  hitboxes are the painter's row rects.
- **The folder tree becomes a model in the pure-data layer.**
  `build_file_tree` and its `TreeRow` leave `src/ui/code_review.rs` for
  `src/session/review.rs` as `file_tree_rows` and `FileTreeRow`, beside the `DiffFile`
  they group and the `pair_hunk` that already lives there. Behaviour-identical: the sort,
  the fold and the preserved indices are the same code.
- **A file's status glyph resolves through one token.** `status_token` names the colour
  role once; `status_color` resolves it for the diff's own header row and
  `files_list_tree` names it for the list's row, so the two rows cannot come to disagree
  about what a rename looks like.
- **The pre-port span builders are retained as the oracle.**
  `the_changed_files_tree_paints_what_the_span_builder_painted` paints the same rows both
  ways and asserts the buffers are equal — every cell, every colour, at a width that fits
  and one that cuts a folder name short.

## The behaviour that changes, stated rather than discovered

The changed-files list scrolls differently. Before: once the list overflowed its column,
the file the diff's cursor is in sat on the **last** visible row, so the list showed the
files *above* it and never the ones after. After: the window opens `min(height / 4, 3)`
rows above that file and clamps at the list's tail, so the files on either side of it are
visible — the rule the tasks pane, the automations band, the file viewer, the session list
and every plugin list already scroll by.

Nothing else moves. The unscrolled frame is unchanged cell for cell, which the retained
oracle asserts rather than claims.

## Non-goals

- **Handing anything over.** `src/ui/code_review.rs` still exists, still draws the diff and
  still draws this list. All five rows of `tests/code_review_pane_handover_gap.rs` keep
  their verdicts, including `no-second-seat-for-the-changed-files-list` — a seat contested
  by two kernel surfaces is not made uncontested by changing how one of them scrolls.
- **Publishing the tree.** `file_tree_rows` is reachable by `session`'s publication layer
  now, and nothing publishes it. A pane that draws this list needs the rows *and* a seat,
  and the seat is the refused row.
- **The nav-key legend.** The kernel still draws it on the column's last row, subtracted
  before the list is painted, exactly as it did.
- **The diff's own window.** `render` still scrolls the diff stream by its own arithmetic
  over visual rows, which is a different problem: a diff row is not one line under wrap or
  side-by-side, and that is `no-resolved-width`'s row.

## Gate

No compile-time gate. `src/ui/plugin_pane.rs` is the view-tree renderer and is in every
build — the `plugins` Cargo feature gates the VM, not the painter — so this pane draws
identically with and without it, which `cargo nextest run --all --no-default-features`
covers.
