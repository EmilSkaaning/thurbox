# Design — the changed-files list's port

## Why the list is published rather than derived

The tempting reading is that the changed-files list is a projection of the diff: every
file has a header row in the stream, so a pane could filter the stream for `file` rows
and have its list. Two facts defeat it, and they are independent:

1. **The stream is bounded** (`MAX_REVIEW_ROWS`, 60 rows over *every* kind), because a
   diff line's node cost is unbounded — its body is one node per syntax token. A review
   of twenty files exhausts that budget inside its first two. A pane filtering the
   stream would therefore list the files it happened to receive, and there is no signal
   in the section that would tell it the list is short.
2. **The order is different.** The stream is the review's reading order; the tree is
   grouped by directory and sorted by path *segment* (`session::review::file_tree_rows`,
   relocated there by ADR-65). Sorting in Luau would compare strings by `strcoll`
   against Rust's byte-wise `str` ordering, which agrees on ASCII and is not guaranteed
   to elsewhere — a divergence that would appear only on a repository with non-ASCII
   paths, in a pane nobody would think to test that way.

So the list crosses, as `number_width` already does, and for the same stated reason:
a quantity computed over the whole review is not derivable from a window onto it.

### Rejected: publish only the files and let the pane build the tree

Halfway: the pane would still sort, and (2) would still bite. The grouping is a model,
and ADR-60 already decided where a model belongs.

### Rejected: publish the rendered rows

The section would carry basenames, status letters, `✓` marks and indentation strings —
which is exactly what `kernel-state` forbids, and what would make the reproduction an
arrangement of the kernel's decisions rather than evidence that a pane can be written.
The pane derives all four from facts.

### Rejected: raise `MAX_REVIEW_ROWS` so the stream contains every file

It would have to rise to whatever the largest review is, which reintroduces the node
budget the bound exists to protect — and would ship sixty times the wire for the one
row kind the list needs.

## Two panes, one plugin

The manifest gains a second `[[panes]]` rather than a second plugin. They read one
published section, they are one surface to a user, and two manifests would mean two
lifecycles, two capability grants and two failure states for a review. The plugin's
`render(paneId)` already takes the pane's id — the entry point was built for this — so
the second pane is a branch, not a second module.

The pane is seated in the **right column** and hidden by default. Not the file-viewer
seat: that is the seat its handover is refused on, and a reproduction sitting there
would place a copy where the original is and preempt the file viewer's pane on the way.
`the_reproduction_claims_neither_the_seat_nor_the_keyboard` asserts the manifest names
neither the seat nor `key_context`, so the port cannot quietly become a handover.

## The builder takes snapshots

`files_list_tree` was `(&CodeReviewState, &[FileTreeRow])` after ADR-65 and is now
`(&[ReviewFileRowSnapshot], Option<usize>)`, mirroring `review_stream_tree`. Two
consequences worth stating:

- the oracle can build the native side from the same fixture rows it publishes, so a
  failure is about the plugin rather than about two hand-written fixtures;
- the native pane now renders from a snapshot of its own state, which is what makes
  "both panes are built from one description" true rather than aspirational.

The bound is the argument that keeps them honest: `file_row_snapshots(limit)` is called
with `MAX_REVIEW_FILE_ROWS` by the publication and `usize::MAX` by the pane. Baking the
bound into the producer would silently truncate the native column; leaving it out would
put an unbounded list on the wire. The difference between the two calls is the one
divergence the port enumerates.

## The empty list has no cursor

`files_list_tree(&[], None)` yields a list with `selected: None`, not `Some(0)`: naming
row zero of a list with no rows is an index into nothing, and the plugin's `ui.list({})`
would have had to invent one to match. The native pane returns early on an empty review
so no frame moves either way.

## Module ownership

Nothing new crosses an architecture boundary. `ReviewFileRowSnapshot` joins
`session::pane_context` beside the other published rows; `file_tree_rows` is already in
`session::review` (ADR-65); `files_list_tree` stays in `ui::code_review`; the Lua
conversion is `plugin::kernel_state`, which already reads `session::pane_context` and
nothing else. `tests/bundled_review_files.rs` is an integration test, so it may see both
`ui` and `plugin` without touching `tests/architecture_rules.rs` — the reason its four
siblings live there.
