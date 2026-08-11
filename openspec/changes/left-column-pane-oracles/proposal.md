# The left column's two panes get an oracle that outlives their native builders

## Why

The session list and the automations pane are the two panes the next handover
attempt targets — and they are the two whose evidence is about to evaporate for
the reason ADR-42 found on the info panel.

Both oracles are **differential**. `tests/bundled_session_list.rs` asserts the
plugin's tree equals `ui::project_list::session_list_tree`'s;
`tests/bundled_automations_panel.rs` asserts it equals
`ui::automations_panel::automations_tree`'s. Both named builders live in the
modules a handover deletes (`src/ui/project_list.rs`,
`src/ui/automations_panel.rs`). So each proof can fail *before* the handover and
not *after* it: with the right-hand side gone the repair that compiles is to drop
the comparison, and what remains is a test that the plugin renders without
erroring — satisfied equally by a pane drawing one wrong row and by one drawing
twenty. `migration/phase-4` already forbids the acceptance snapshots as a
substitute, and none of the seven holds a cell of either pane (all are captured
with no active session).

`migration/phase-4` states the recording rule and the info panel obeys it. Two
things push these two panes to the front of the queue rather than leaving each
recording to "its own handover":

- **A refused handover does not produce the recording.** The attempt on each of
  these panes concludes that the pane stays native, and a conclusion is not a
  change that records anything. So the hole survives every refusal and the next
  attempt inherits it — while the recording is only *provable* now, from a
  builder that still exists.
- **The recorder is one program, and it is currently one pane's private
  module.** Its exhaustiveness over the view tree is what stops the compact form
  from omitting a fact (ADR-42), and that guarantee is worth exactly as much as
  the number of copies of it: three copies are three formats that can drift, and
  a reviewer comparing three panes' recordings would be comparing three
  renderings.

## What Changes

- **One recorder, shared.** The line-per-node renderer moves out of
  `tests/bundled_info_panel.rs` into a `tests/view_tree_record/` module the pane
  oracles include, so the exhaustive destructuring that makes a new view-tree
  field a compile error exists once.
- **The session list's tree is recorded**, one snapshot per comparison case,
  generated from `ui::project_list::session_list_tree`.
- **The automations pane's tree is recorded**, one snapshot per comparison case,
  generated from `ui::automations_panel::automations_tree`.
- **Both edges are asserted while both sides exist**, as the info panel's oracle
  does: the recording equals the native tree, and the plugin equals the native
  tree. Their conjunction is what a later handover inherits.
- **Non-vacuity is demonstrated per pane**, by perturbing each side in turn and
  recording the observed failure.

## Impact

- Affected specs: `migration/phase-4` (one MODIFIED requirement, one ADDED).
- Affected code: `tests/bundled_session_list.rs`,
  `tests/bundled_automations_panel.rs`, `tests/bundled_info_panel.rs` (the
  recorder moves out of it), and new snapshots under `tests/snapshots/`.
- No `src/` change, no behaviour change, no deletion. Both native panes are still
  what the interface draws, `tests/teardown_gate.rs` is untouched, and both rows
  stay blocked.
- **Test count is unchanged.** The recorded edge is assertions inside each
  pane's existing per-case loop, not new tests — stated plainly because "the
  suite grew" is the usual evidence a proof was added, and here it is absent by
  construction. What changed is what those two tests can fail for, shown by
  perturbation.
- Confined to `tests/`, so the `--no-default-features` build is untouched: every
  file involved is `#![cfg(feature = "plugins")]` and does not compile there.
