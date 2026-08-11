# Design

## What this change is

A verdict, made executable. No source file changes; the artefact is a gate whose
rows are re-derived from the tree, so the refusal cannot quietly expire.

The shape is the sibling gates' (`tests/tasks_pane_input_gap.rs`,
`tests/automations_pane_handover_gap.rs`, `tests/session_list_pane_handover_gap.rs`):
a `BLOCKERS` table, one row per requirement, each tagged `Structural` /
`Vocabulary` / `Wiring`, each with a probe that reads the declaration it is about.
Deliberately the same shape, because five gates that answer the same question in five
different ways are five things to learn.

## The rows, and what is new in them

Eleven rows. Three are this pane's own findings; the rest are shared walls this pane
happens to hit hardest.

| Row | Kind | New here? |
|---|---|---|
| `no-central-seat` | structural | no — ADR-38, ADR-43. New only in that the *whole* pane wants the seat |
| `no-second-seat-for-the-changed-files-list` | structural | **yes** — the first surface needing two seats at once |
| `keys-are-a-capture-not-actions` | structural | **yes** — the first pane with no `Action` to name |
| `no-review-write` | structural | no — the seam's fifth absence |
| `no-retarget-operation` | structural | **yes** — the first key whose effect is running `git` |
| `no-export-operation` | structural | **yes** — clipboard and a session's pty, neither reachable |
| `no-cursor-write` | structural | narrower than the session list's, which is the point |
| `no-resolved-width` | structural | no — ADR-31 named it; three keys and an ellipsis depend on it |
| `mouse-carries-no-column` | wiring | **yes** — a coordinate, not a target kind |
| `no-anchored-overlay` | vocabulary | §10's bottom-anchored row in a second shape |
| `no-in-pane-text-field` | vocabulary | a caret, and a third styling mode over already-styled runs |

## Decisions

### The verdict is a gate, not a document section

**Chosen.** A table with probes, plus one test per finding.

*Rejected: recording it only in `docs/PHASE4-PANE-READINESS.md`.* A verdict in
markdown is a fact about a build that expires without telling anyone — the reason
every earlier refusal on this branch is a gate.

*Rejected: folding the rows into `tests/teardown_gate.rs`.* That table answers
whether `src/ui/code_review.rs` may be deleted. One table answering two questions
produces failures that do not say which question moved.

### The two seats are separate rows

**Chosen.** `no-central-seat` and `no-second-seat-for-the-changed-files-list`.

*Rejected: one "seats" row.* They close differently. A central slot is one addition
to `PaneSlot` plus a central-pane branch that names a plugin pane. The second seat is
a *second pane* — its own focus, its own keys, its own selection driving the first
pane's scroll — and a plugin declaring two panes that must be focused and navigated as
one surface is a question no existing row asks. Collapsing them would make the second
look like a detail of the first.

### The keyboard row is stated as "no action to name", not "no keys"

**Chosen.** The probe checks three things: `KeyContext` names no review, the captures
run before `lookup_in`, and `focus_key_context` names no plugin pane.

*Rejected: "a plugin pane receives no scoped action", the session list's phrasing.*
True but weaker, and it misdescribes the fix. For the session list the actions exist
and resolve in the wrong scope; here they do not exist, so the first step is a
**keybinding-vocabulary** change — turning a capture into scoped actions — which is
upstream of anything plugin-facing. A row that read like the session list's would put
the work in the wrong order.

### The mouse row separates a missing target kind from a missing coordinate

**Chosen.** One row, with the distinction in its `stands` and pinned by its own test.

*Rejected: one row per missing target (buttons, scrollbar, wheel, picker).* Four rows
that all close with one wider event, plus a fifth that does not, reads as five
problems where there are two.

*Rejected: calling the whole row structural.* Buttons and a scrollbar are events the
host could deliver today with no new plugin-facing concept — `Wiring` by the sibling
gates' definition. Calling it structural would claim the model forbids it, and it does
not. The column half is what is hard, and it is stated inside the row rather than
promoted to a kind.

### The cursor row says it is *narrower* than the session list's

**Chosen.** Same id (`no-cursor-write`), explicitly compared, with a test asserting
the difference.

*Rejected: repeating the session list's row verbatim.* Two rows spelled alike with
very different prices read alike, which hides the cheapest available next step: the
review's cursor is a row inside a view the user already opened, read by the review and
the changed-files highlight and nothing else, whereas the session list's cursor *is*
the application's active session.

### No capability is added

**Chosen.** The change touches no source file.

*Rejected: adding `review-write` alongside the verdict.* It would be the fourth
capability in the host with **zero consumers** — the defect the earlier gates
identified in `input`, `tasks-write` and `automations-write`, which existed before any
bundled plugin declared them. And it would be premature: a review write without the
two seats and without the cursor gives a pane the power to mark a file reviewed while
being unable to say which file the user is looking at.

*Rejected: declaring `input` on the bundled plugin so its pane can be focused.* The
pane is not focusable today precisely because it declares no `input`
(`PluginPane::is_focusable` = visible && accepts_input), and that is the honest state:
keys with nothing to act on would be a pane that takes a keystroke and drops it.

## Consequences

- The gate runs in both feature configurations, like its siblings, because its probes
  read source text rather than compiled behaviour.
- A change that adds a central slot, a review write, a cursor write, a review key
  context or a wider click event fails the gate and is told which row moved and what
  to revisit.
- The teardown inventory is unchanged: `src/ui/code_review.rs` was already protected
  and stays so.
- The ordering the table implies: the pane's keys become scoped actions, then the
  narrow cursor write, then the two seats. Nothing else is reachable before those.
