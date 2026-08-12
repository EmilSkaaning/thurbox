# The code review's changed-files list, reproduced by the bundled plugin

## Why

The code review is drawn as **two** panes: the diff stream in the central pane and
the changed-files list in the file-viewer column, each with its own focus and its own
keyboard. ADR-44 reproduced the first. The second has no reproduction at all, and its
handover row (`no-second-seat-for-the-changed-files-list`) is the first of the five
that refuse this surface.

`migration/phase-4` requires the reproduction to come **before** the handover, and its
recording to be derived from the kernel's own builder while that builder still exists
(ADR-48) — after the deletion the only baseline available is the plugin itself, which
would make a plugin defect the expectation. So the port is a change of its own.

The v1 behaviour being extended is the publication. The `review` capability publishes
the diff's **row stream**, which is neither the same list nor a superset of it: the
stream is bounded at 60 rows, so a large review's later files never appear in it at
all, and its order is the review's (a folded file collapses to a header, comments
interleave) rather than the tree's (grouped by directory, sorted by path segment). A
pane deriving the changed-files list from it would draw a prefix of the files, in the
wrong order.

## What Changes

- **The published review section gains a second list.** `ReviewSnapshot` carries
  `file_rows` — folder headers and file rows with depth, path, status name, counts and
  the reviewed flag — plus `file_cursor`, the row holding the file the diff's cursor is
  in. Bounded separately by `MAX_REVIEW_FILE_ROWS`, because a changed-file row costs a
  fixed handful of nodes where a diff line's body costs one per token.
- **No capability is added.** The rows cross in the `review` section the diff already
  reads, and the bundled plugin's manifest still declares `["render", "review"]`. A
  section grows; a grant does not.
- **The kernel's tree builder takes the published rows.**
  `ui::code_review::files_list_tree` moves from `&CodeReviewState` to
  `(&[ReviewFileRowSnapshot], Option<usize>)`, mirroring `review_stream_tree`, so both
  panes are built from one description of what a pane receives.
- **The bundled plugin gains a second pane** (`code-review` / `files`) drawing that
  list: the folder rows, the basenames, the status letters, the reviewed marks and the
  counts, with the selection appearance on the cursor's file. Seated in the right
  column and hidden by default, like the diff's reproduction.
- **`tests/bundled_review_files.rs`** asserts tree equality against the kernel builder
  and records seven cases, covering both row kinds, all four statuses, an unknown
  status, an absent cursor, a truncatable directory name and multi-byte names.

## The one divergence, enumerated

The publication is bounded and the native pane is not. `CodeReviewState::file_row_snapshots`
takes its limit as an argument: the publication passes `MAX_REVIEW_FILE_ROWS`, the
native pane passes `usize::MAX`. So a review with more than 400 changed files lists
them all natively and the first 400 in the reproduction. That is the wire's bound, not
the pane's, and it is stated here rather than discovered at the 401st file.

## Non-goals

- **Handing anything over.** `src/ui/code_review.rs` still draws both panes. All five
  rows of `tests/code_review_pane_handover_gap.rs` keep their verdicts, and
  `no-second-seat-for-the-changed-files-list` in particular: a copy in the right column
  does not resolve a seat contested in another.
- **Claiming the seat or the keyboard.** The manifest names neither, and
  `the_reproduction_claims_neither_the_seat_nor_the_keyboard` asserts both absences —
  a reproduction that took the keys would take them off the list the user can see.
- **The nav-key legend.** It is the kernel's row, drawn outside the native pane's list,
  and it is not reproduced: a plugin naming a rebindable key would go stale. Which
  chrome shape carries it is the handover's problem.
- **Mouse, scrolling and the `/` find.** The reproduction draws; the native list is
  still what a click, a wheel and a search act on.

## Gate

The plugin half is behind the `plugins` Cargo feature, which has been in the default
set since Stage B (ADR-40), so it ships. The publication and the tree builder are in
every build; `cargo nextest run --all --no-default-features` covers the kernel half,
and the oracle is `#![cfg(feature = "plugins")]` like every other bundled-pane test.
