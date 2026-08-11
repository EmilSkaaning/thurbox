# Refuse the code review's handover, with the reasons as a gate

## Why

The code review's **document** is now reproduced whole (the sibling change): every
row kind the native pane lists, pinned to the untouched renderer row by row. The
next step is the handover — drawing the plugin's pane instead of
`src/ui/code_review.rs`. It does not happen, and this change is that verdict in
executable form.

Three of the reasons are already recorded against other panes (a seat, a cursor, a
write). Three are this pane's own, and they are why it is the furthest from a
handover rather than the closest:

1. **It is two panes, not one.** The diff owns the **central** pane, and the
   changed-files list owns the **file-viewer column** — a second focus
   (`InputFocus::ReviewFiles`) with its own keys, force-shown by `App::layout_for`
   whenever a review is open. Every earlier refusal needed one seat. This needs two
   at once, in two different columns, for one plugin.
2. **Its keyboard is not in the keybinding system at all.** `KeyContext` has six
   members and none is a review. `handle_code_review_key` and
   `handle_review_files_key` are hardcoded captures keyed on `self.focus`, run
   *before* the global lookup. So unlike the tasks, automations and session-list
   refusals — each of which could at least name the scoped `Action`s a plugin
   binding would have to replace — there is **no action to name**. The pane's keys
   are not rebindable today, so `keybindings.json` cannot restore them either, and
   the F1 editor has never listed them.
3. **Its mouse surface is wider than the row channel.** The pane is documented
   mouse-first. A plugin pane's click reaches `onClick(paneId, row)` — a row and
   nothing else. The review also has footer buttons (eleven of them), a draggable
   scrollbar, a wheel, target-picker entries, and — uniquely in thurbox — a click
   whose **column** decides which side of a paired row a comment attaches to
   (`CodeReviewState::click_side`). A row index cannot carry a column.

One row is where this pane is *closer* than the session list, and saying so is the
point of a gate rather than a verdict: the review's cursor is the review's own row,
not the application's active session. A capability letting a pane name the row it is
on would be far narrower here than the grant the session list's cursor would need.
That is the cheapest next step, and it is recorded with the order to take it in.

## What Changes

- **`tests/code_review_pane_handover_gap.rs`** (new): one row per requirement the
  handover needs and does not have, each re-derived from the source and each tagged
  `Structural` / `Vocabulary` / `Wiring` — the three kinds the sibling gates use, so
  the pane's remainder sorts into the same buckets the rest of Phase 4 does.
- **Findings pinned as their own tests**, so a failure names the argument rather
  than only the rule: that the review needs two seats; that its keyboard resolves no
  `Action`; that the mouse channel carries a row where the pane needs a column; and
  that the cursor row is narrower than the session list's.
- **Non-vacuity asserted in both directions**: the verdict is derived from the rows,
  so a table where everything landed permits the handover.
- **The native pane stays**, and the gate asserts it — every row above is a claim
  about a handover that has not happened.
- **Documented**: `docs/PHASE4-PANE-READINESS.md` §20 and `docs/ARCHITECTURE.md`
  ADR-45.

## Impact

- New: `tests/code_review_pane_handover_gap.rs`.
- Docs: `docs/PHASE4-PANE-READINESS.md`, `docs/ARCHITECTURE.md`.
- Spec: `migration/phase-4` gains the requirements a two-seat pane and a
  capture-keyed keyboard imply.

Not changed: no source file, no capability, no manifest. Deliberately — this change
adds no capability with no consumer, which is the defect three of the earlier gates
were written to avoid.

The teardown inventory is untouched: it answers whether
`src/ui/code_review.rs` may be deleted, which is already no and stays no.
