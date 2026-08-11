# Design

## The two decisions

1. **What the recording is** — settled by ADR-42 and unchanged here: a
   line-per-node rendering produced by the single shared recorder in
   `tests/view_tree_record/`, generated from the native builder, asserted in the
   order recording → legible → exact. The three panes recorded here reuse it
   verbatim; nothing about the format is re-decided.
2. **What makes the recording owed, and who checks** — the part this change
   settles. Reproduction makes it owed; `tests/teardown_gate.rs` checks.

## Why the trigger moves from "attempt" to "reproduction"

The existing rule fires when a handover is *attempted*. Three facts make that the
wrong trigger:

- It is unobservable. There is no artefact in the tree that says "an attempt
  concluded", so no test can hold anyone to it. The three recorded panes were
  recorded because two changes chose to; the three unrecorded ones were skipped
  because three changes did not.
- It fires too late. §15, §16 and §20 each concluded before the rule existed, so
  the rule's own backlog is the majority of the panes it governs.
- It is not the moment of maximum information. At reproduction the plugin and the
  native builder both exist and are known to agree — the ideal moment to freeze
  that agreement. At attempt time they still both exist, so nothing is gained by
  waiting, and if the attempt is deferred indefinitely, nothing is recorded at all.

Reproduction is observable (a bundled plugin directory plus an oracle), is the
earliest moment the recording can be proved, and is already the trigger for every
other pane obligation in `migration/phase-4`.

## Where the check goes, and the alternatives rejected

**Chosen: a fourth conjunct in `tests/teardown_gate.rs`'s `pane()` probe, plus a
positive assertion that every reproduced pane records its tree.**

The gate is the one place that decides whether a native renderer may be deleted,
and the failure this guards against is precisely a deletion. Putting the condition
anywhere else leaves the gate — the thing a handover author actually runs — green
on a handover that destroys its own oracle. The conjunct is re-derived from the
tree like the other three, so the verdict cannot go stale, and it is read from
source text rather than from `cfg!`, so it means the same thing under
`--no-default-features` (where none of the pane oracles compiles at all).

The positive assertion is separate on purpose. The conjunct alone only fires when
someone *also* stops drawing the native pane; a pane reproduced today with no
recording would sit blocked for the right reason (condition 2) and the missing
recording would stay invisible until the moment it was too late to add. The
assertion fails now, which is when it is actionable.

Rejected alternatives:

| Alternative | Why not |
|---|---|
| Leave it a spec requirement with no probe | This is the third time this phase that a probe had to be tightened rather than a verdict flipped (§10's write-shaped binding, §11's `Fill`, §14's two-condition handover). A requirement that only a reviewer enforces is the same shape as the convention that already failed for three panes |
| A separate test file, e.g. `tests/pane_oracles.rs` | It would pass while `teardown_gate` permitted the deletion, and the author of a handover reads the gate. Two gates disagreeing about whether a pane may go is worse than one gate that is complete |
| Assert on `.snap` file *contents* (e.g. that they name the pane) | The contents are the oracle's business, and `insta` already fails when they move. The gate's question is structural: does a recording exist and is the oracle wired to it |
| Derive the oracle path from the pane id | It is not derivable — `tasks-plugin` → `bundled_tasks_panel.rs`, `session-list-plugin` → `bundled_session_list.rs`, `info-panel-plugin` → `bundled_info_panel.rs`. Guessing across three spellings would make the probe's failure mode "file not found" for a renamed test rather than "no recording", so the mapping is a table field beside `native_module`, which the row already carries for the same reason |
| Record only the panes whose handover is next | The premise of the change is that "next" is not knowable and has already been wrong three times |
| Record from the plugin (much cheaper — no case-by-case native call) | Forbidden by `migration/phase-4`, and it is the whole point: a plugin defect would become the expectation |

## Why `Option<&'static str>` for the oracle, not a required field

Global search has no bundled plugin and no oracle, by a recorded verdict
(`tests/global_search_pane_gap.rs`, `migration/phase-4`). A required field would
force either a fabricated path or a sentinel string; an `Option` says what is true
— there is no oracle because there is nothing to constrain. The well-formedness
test then reads: a pane row whose bundled plugin exists MUST name an oracle. That
turns a future bundled global-search plugin into a failure that asks for its
recording, which is the behaviour wanted.

## The one thing the probe cannot check

Whether the recording was generated from the *native* builder rather than from the
plugin. The tree holds a `.snap` file and an `insta::assert_snapshot!` call; it
does not hold the provenance of the bytes. Two things cover that gap, and neither
is the gate:

- the recorded edge is asserted **against the native tree** in the same loop, so a
  recording taken from the plugin fails the moment the two disagree — which is the
  moment it would matter; and
- the per-pane perturbation of the *native* side, run and recorded in `tasks.md`,
  demonstrates that the recording tracks the native builder.

Stating this is the point: the gate makes the recording's *existence* mechanical
and leaves its *provenance* to an assertion plus a demonstrated perturbation. A
gate that claimed both would be claiming something it cannot see.

## Module ownership and the architecture rules

Nothing lands under `src/`. All four touched files are in `tests/`, which is not
part of the library's module graph, so `tests/architecture_rules.rs` needs no
allowlist change — the same reason each pane oracle may see both `ui::*` and
`plugin::PluginHost` when no module under `src/` may.

`tests/teardown_gate.rs` stays free of `#[cfg(feature = "plugins")]`: it reads
source text, so its verdicts are identical in both build configurations. The new
conjunct reads `tests/bundled_*.rs` as text for the same reason.

## Case selection

Each pane records the cases its existing `cases()` already enumerates — no new
fixtures, and the recorded edge goes into the loop that already compares them. Two
categories of fixture stay outside `cases()` and therefore unrecorded, which is the
existing shape rather than an exclusion invented here:

- **the enumerated divergences**, which assert the two panes *differ*
  (`a_title_wider_than_the_column_is_fitted_by_the_kernel_only`,
  `the_search_bar_is_out_of_scope_but_its_effect_on_the_rows_is_not`,
  `the_out_of_scope_surface_is_absent_rather_than_approximated`). Recording one
  side of a deliberate difference would pin the difference, not the pane; and
- **the scroll and bounds fixtures** (`a tall tree`, `a tall diff`, `overflowing`,
  `a full publication`, `an ordinary diff`), which exist to be *painted* at a
  forced size or to exceed a host budget. Their claim is a frame or a refusal, not
  a tree, and a recording of a several-thousand-node tree is the unreviewable dump
  ADR-42 rejects.

Each pane's header states which of its tests are unrecorded and why, so a reader of
the file does not have to reconstruct the rule from `design.md`.
