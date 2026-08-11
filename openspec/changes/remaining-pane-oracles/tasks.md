# Tasks

## 1. Record the tasks pane

- [x] `tests/bundled_tasks_panel.rs`: `mod view_tree_record;`, a
      `Case::snapshot_name` derived from the case name (never written twice, so a
      renamed case cannot keep asserting against another's recording), and in the
      existing per-case loop of `the_plugin_builds_the_native_panes_view_tree` the
      recorded edge (`native == snapshot`) asserted **first**, then the legible
      comparison, then the exact one.
- [x] Extend the file header with why the enumerated divergence and the scrolling
      frame test stay unrecorded.
- [x] Generate with `INSTA_UPDATE=always cargo test --test bundled_tasks_panel`,
      then read each snapshot and confirm it is the tasks pane — the checkbox
      glyph per status, the selection appearance, the `⇄` linked marker, the
      underlined matched runs, the multi-byte titles.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E
      'test(bundled_tasks_panel)'`; `git status` shows no `.snap.new`.

## 2. Record the file viewer

- [x] `tests/bundled_file_viewer.rs`: the same three assertions in its per-case
      loop, plus `snapshot_name`.
- [x] Generate, then read each snapshot and confirm it is the file tree — the
      indentation per depth, both marker sets, the collapsed-versus-expanded
      chevron, the muted rows a search excluded, the scrollbar declaration on the
      list node.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E
      'test(bundled_file_viewer)'`; no `.snap.new`.

## 3. Record the code review

- [x] `tests/bundled_code_review.rs`: the same three assertions in its per-case
      loop, plus `snapshot_name`.
- [x] Generate, then read each snapshot and confirm it is the review's document —
      a file header's rule, chevron, status letter and counts; a hunk's `@@`
      range; the gutter and the row tint on an insertion and a deletion; each
      syntax token's role; a comment's classification badge; the summary heading.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E
      'test(bundled_code_review)'`; no `.snap.new`.

## 4. Make the recording a gate condition

- [x] `tests/teardown_gate.rs`: add `oracle: Option<&'static str>` to
      `Replacement` beside `native_module`, name each reproduced pane's oracle
      file in its `pane()` row, and add the fourth conjunct to
      `pane_is_handed_over` — pure over the conditions, like the other three, so
      the case the tree cannot exhibit is still testable.
- [x] Add `pane_oracle_records_the_native_tree(root, id)`: the oracle file names
      the shared recorder **and** asserts a snapshot, and at least one
      `tests/snapshots/<stem>__*.snap` exists. A row with no oracle answers
      `false`.
- [x] Add `every_reproduced_pane_records_its_native_tree`: for each pane row whose
      bundled plugin exists, the row names an oracle and that oracle records. This
      is the assertion that fails *now* rather than at deletion time.
- [x] Extend `the_build_condition_holds_and_still_gates_a_handover` (or a sibling)
      with the pure case: `pane_is_handed_over(true, false, true, false)` is
      `false` — a pane whose oracle is differential is not handed over.
- [x] Update the `pane()` doc comment to four conditions and say what condition 4
      rules out that the other three do not: a deletion that is invisible in a
      running binary.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — all
      pane rows still blocked; and the same command with
      `--no-default-features`, since the gate must mean the same thing there.

## 5. Prove all three oracles can fail, in both directions

All six perturbations were run and the failures observed. Recorded results:

| Perturbation | Observed |
|---|---|
| `tasks/init.luau`: `MATCHED` drops `underline` | recorded conjunction fails, `a running search`, `line 4: native text "h" accent bold underline / plugin text "h" accent bold` |
| `ui::tasks_panel`: glyph run gains a space | recorded edge fails first — `insta` diff on `one-row-per-status`, three rows |
| `file-viewer/init.luau`: expanded marker `▾`→`▿` | recorded conjunction fails, `an expanded tree with a collapsed directory`, line 3 |
| `ui::file_viewer`: indent `"  "`→`"   "` | recorded edge fails — `insta` diff on every nested row |
| `code-review/init.luau`: `issue` badge `danger`→`warning` | recorded conjunction fails, `each comment classification`, line 3 |
| `ui::code_review`: badge arrow `▸`→`»` | recorded edge fails — `insta` diff on all five badges |
| the gate: tasks' eleven recordings moved aside | `every_reproduced_pane_records_its_native_tree` fails naming `tasks-plugin` |

- [x] Perturb `src/plugin/bundled/tasks/init.luau` (one style fact), run the test,
      confirm the recorded conjunction fails and names the case and the line.
      Revert; record the observed diff here.
- [x] Perturb `ui::tasks_panel::tasks_tree`'s presentation step, confirm the
      *recorded* edge fails — which is what makes the recording a statement about
      the native pane rather than a copy of it. Revert; record the diff.
- [x] The same two perturbations for the file viewer
      (`src/plugin/bundled/file-viewer/init.luau`, then `ui::file_viewer::file_tree`).
- [x] The same two for the code review
      (`src/plugin/bundled/code-review/init.luau`, then
      `ui::code_review::review_stream_tree`).
- [x] Prove the gate's new condition is not vacuous: remove one pane's recorded
      assertion (or its snapshots), confirm
      `every_reproduced_pane_records_its_native_tree` fails naming that pane.
      Revert.

## 6. Documentation

- [x] `docs/PHASE4-PANE-READINESS.md`: a section recording that the recording
      obligation moved from an attempt to a reproduction, why the earlier trigger
      could not fire, and that the gate now carries it.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the fourth handover condition, with
      what it rules out.
- [x] `docs/ARCHITECTURE.md`: an ADR for the trigger move and the gate conjunct.

## 7. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — every pane
      row still blocked; this change hands nothing over
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
- [x] `openspec validate remaining-pane-oracles --strict`
