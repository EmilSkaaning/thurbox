# The last three pane oracles get a recording, and the gate stops trusting a convention

## Why

Six panes are reproduced by a bundled plugin. Three of their oracles assert
against a recording; three still assert only against a builder the handover
deletes.

`tests/bundled_tasks_panel.rs` compares the plugin's tree with
`ui::tasks_panel::tasks_tree`, `tests/bundled_file_viewer.rs` with
`ui::file_viewer::file_tree`, and `tests/bundled_code_review.rs` with
`ui::code_review::review_stream_tree`. Every one of those three builders lives in
the module its pane's handover removes (`src/ui/tasks_panel.rs`,
`src/ui/file_viewer.rs`, `src/ui/code_review.rs`). So each proof can fail *before*
the handover and not *after* it: with the right-hand side gone, the repair that
compiles is to drop the comparison, leaving a test that the plugin renders without
erroring — satisfied equally by a pane drawing one wrong row and by one drawing
twenty. That is exactly the class ADR-42 named on the info panel and
`migration/phase-4` already forbids; the acceptance snapshots are no substitute,
and none of the seven holds a cell of any of these three panes.

**The rule that should have caught this already exists and did not fire.**
`migration/phase-4` says a pane whose handover is *attempted* is owed its
recording before the attempt concludes. The tasks pane's attempt (§15), the file
viewer's (§16) and the code review's (§20) all concluded — refused — before that
rule was written, so all three inherited the hole the rule exists to prevent. The
rule's trigger is also the wrong one: it fires on an *attempt*, which is a human
decision to begin work, and nothing checks it. A port that is never attempted for
a year keeps a differential oracle, and the change that finally deletes the native
builder is the change least able to notice.

So two things are wrong, and the second is why the first recurred:

- three panes have no recording, and the only moment one can be *proved* to be the
  native pane's is one in which that builder still exists; and
- the requirement is enforced by convention. Nothing in the tree fails when a
  pane's oracle is purely differential, so the next handover attempt can satisfy
  every executable condition in `tests/teardown_gate.rs` and still delete the
  evidence along with the code.

## What Changes

- **The tasks pane's tree is recorded**, one snapshot per comparison case,
  generated from `ui::tasks_panel::tasks_tree`.
- **The file viewer's tree is recorded**, generated from
  `ui::file_viewer::file_tree`.
- **The code review's tree is recorded**, generated from
  `ui::code_review::review_stream_tree`.
- **Both edges are asserted while both sides exist** in all three, as the three
  recorded panes already do: the recording equals the native tree, and the plugin
  equals the native tree. All three use the shared `tests/view_tree_record`
  recorder, so the exhaustive destructuring that makes a new view-tree field a
  compile error still exists once.
- **The recording becomes the handover gate's fourth condition.** `pane()` in
  `tests/teardown_gate.rs` gains a conjunct: a pane is handed over only when its
  oracle carries a recorded expectation. It is re-derived from the tree like the
  other three, so a pane whose oracle is still differential is recorded *blocked*
  and its native renderer stays protected — the rule stops depending on whoever
  writes the handover remembering it.
- **Non-vacuity is demonstrated per pane**, by perturbing each side in turn and
  recording the observed failure.

## Non-goals

- **No pane is handed over and nothing is deleted.** All six native renderers are
  still what the interface draws; every pane row in `tests/teardown_gate.rs` stays
  blocked, and the new condition changes no verdict — it is satisfied by all six
  reproduced panes once this lands, so each row remains blocked by condition 2
  alone.
- **Global search gains no oracle.** It is recorded structurally unportable
  (`tests/global_search_pane_gap.rs`), has no bundled plugin, and its row is
  blocked by condition 1; the new condition reads `None` for it rather than
  inventing a recording for a pane that will not be handed over.
- **No change to what any pane draws.** No `src/` behaviour moves; the plugin
  sources and the native renderers are untouched.
- **The enumerated-divergence tests stay unrecorded.** They assert *inequality*
  between the two panes, so a recording of one side would pin a difference rather
  than a pane.

## Impact

- Affected specs: `migration/phase-4` (one MODIFIED requirement),
  `migration/teardown` (one MODIFIED requirement).
- Affected code: `tests/bundled_tasks_panel.rs`, `tests/bundled_file_viewer.rs`,
  `tests/bundled_code_review.rs`, `tests/teardown_gate.rs`, and new snapshots
  under `tests/snapshots/`.
- No compile-time gate is involved on the oracle side: all three pane oracles are
  `#![cfg(feature = "plugins")]`, so the `--no-default-features` build does not
  compile them. `tests/teardown_gate.rs` is deliberately feature-free and reads
  the source tree, so its new condition means the same thing in both
  configurations.
- Test count grows by one (`every_reproduced_pane_records_its_native_tree`); the
  recorded edges are assertions inside each pane's existing per-case loop, not new
  tests.
