# The file viewer is not handed over, and its verdict is re-derived

## Why

The file viewer was the second pane proposed for this work, on the expectation that
handing it over would need the `files` capability **widened** — a filesystem read for
`l`/`Enter` on a directory, a process launch for `Enter` on a file. That expectation is
now wrong, and leaving it recorded would send the next attempt looking for the widest
grant in the host.

`tests/file_viewer_pane_input_gap.rs` records six rows, five of them structural, all of
them true of one question: *what would a **plugin's own** keys need to drive this pane?*
ADR-51 answered a different question — a pane may declare that it **is** thurbox's file
viewer, and the kernel keeps resolving `KeyContext::FileViewer` and performing those
actions itself. So the kernel still reads the directory and still launches the editor,
and **no capability is widened at all**. Five of the six rows stop being handover
requirements.

What is left is three *decisions*, none of them a grant, and this change records them
where the old rows were rather than in prose that expires:

1. **The search bar.** Three rows of bordered chrome below the tree, carrying a query,
   a caret and a `(2/5)` match counter — all kernel state. ADR-53 established seat
   chrome for **one** row (the tasks pane's hints); this is the same mechanism at three
   rows with a cursor cell in it.
2. **The module is the pane's model, and the home of `visible_window`.**
   `src/ui/file_viewer.rs` is 1601 lines: `FileNode`, `FileRow`, `Activation` and
   `FileViewerState` (the expansion set, the cursor, the search, the directory reads)
   plus `enumerate_paths` — and `visible_window`, the rule **every plugin list** and
   three native panes scroll by. Deleting the renderer means relocating both, at five
   call sites in four modules.
3. **The column has a second kernel occupant.** While a code review is open,
   `RegionId::FileViewer` is force-shown and holds the review's *changed-files list* —
   its own focus (`InputFocus::ReviewFiles`), its own keys — which ADR-45 records as
   wanting that region specifically. A `PaneSlot::FileViewer` claim has to yield to it,
   and that rule is not written.

None of the three is refused in principle. All three are unmade decisions, and a
half-made one would leave a column that is empty while a review is open, or a pane
whose scroll window stopped working for every *other* plugin.

## What Changes

- **The gate is re-verdicted, row by row, and becomes a handover gate.** Three of its
  rows close, each with a probe that derives the new fact rather than being deleted:
  `no-view-write`, `no-filesystem-read` and `no-process-launch` are closed **as
  handover requirements**, and each keeps asserting the half that still matters — for
  the filesystem row, that `Capability::Files` is *still* narrow and no binding reads a
  directory. That assertion is the point: the row now proves the grant was not needed
  rather than that it is missing.
- **Two rows are re-scoped.** `sub-mode-keys-are-not-rebindable` and `no-query-write`
  describe the `/` sub-mode, which is kernel state before and after a handover — so
  they are properties of the pane rather than blockers, and say so.
- **`no-frame-node` becomes the search bar's row**, with the mechanism named: seat
  chrome exists for one row, and this needs three plus a caret.
- **Three rows are added**, one per unmade decision: `no-file-viewer-seat`,
  `the-module-is-the-model-and-the-window` and
  `the-column-has-a-second-kernel-occupant`. The second was already asserted as a
  standalone test; it is promoted into the table, because it is now one of the three
  things that decides the verdict rather than a footnote about cost.
- **The verdict test asserts the shape of the remainder**: nothing outstanding is a
  *capability*, which is the finding this change exists to keep true.

## Non-goals

- **The pane is not handed over.** `src/ui/file_viewer.rs` stays, and it is still what
  `src/app/view.rs` draws. The teardown gate's `file-viewer-plugin` row stays blocked.
- **No capability is widened.** Not `files`, not a new `fs`. The brief for this work
  asked for the minimum widening the pane needs; the answer is **none**, and the gate
  is what keeps that answer honest rather than a sentence in a document.
- **No seat is added.** `PaneSlot` gains nothing here: a seat whose claim has no rule
  for the review's changed-files list would break the review the first time someone
  used both.
- **The reproduction is untouched.** `tests/bundled_file_viewer.rs` and the ten
  recordings are unchanged; the plugin still draws the pane's tree and its scroll track.

## Impact

- Affected specs: `migration/handover` (one ADDED requirement).
- Affected code: `tests/file_viewer_pane_input_gap.rs` only.
- Docs: `docs/ARCHITECTURE.md` (ADR-54), `docs/PHASE4-PANE-READINESS.md` §29,
  `docs/PHASE6-TEARDOWN-READINESS.md`.
- No behaviour change, no schema change, no dependency change.
