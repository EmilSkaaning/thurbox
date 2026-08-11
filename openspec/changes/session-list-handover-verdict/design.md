# Design

## 1. Why a refusal is a change at all

Three agents have tried to delete panes and stopped. What made the difference on the
three that landed was that each refusal before them left behind *rows* — executable,
re-derived from the source, tagged by kind — so the next attempt started from a table
rather than from a re-reading of the tree.

This change is that, for the last pane in the left column. The verdict is no; what is
new is **why**, because the reason moved: it used to be the keys (ADR-43), ADR-51
answered that, and what is left is a windowing rule and a module. Neither was in the
gate's table.

## 2. The window: what exactly is derived from `ListState`

Stated as four separate behaviours, because a reader who thinks "the plugin scrolls
too" will otherwise conclude this is a wiring detail.

| Behaviour | Native | A seated plugin pane |
|---|---|---|
| Which rows are on screen | ratatui's sticky `offset()`, over **items** | `visible_window(len, cursor, height)`, over **children** |
| `▲ N` / `▼ N` indicators | `render_scroll_indicators_variable`, from `offset()` and per-item heights, painted **on the block border** | nothing; the catalogue has no chrome node |
| Click hitboxes | computed *after* the stateful render, from the offset ratatui really used; a two-line item is **one** hitbox | one hitbox per child, so a header is separately clickable |
| The pending-spawn placeholder | inserted into the **items** vector at a computed index, shifting every later item | nothing published says a row is a spawn in flight |

And the row counts differ: the native pane's item list folds a repo-group header into
the row *below* it, so 8 sessions in one group is 8 items and 9 lines, while the plugin's
list is 9 children and its declared cursor index counts the header
(`the_two_panes_window_a_long_list_by_different_rules` asserts exactly this today).

So "both keep the cursor visible" is true and insufficient: at any height where the list
overflows the two panes show **different sessions**. For the pane that is thurbox's
primary navigation, and whose selection drives what the central pane, the info column,
the file viewer and the code review are all showing, that is not an enumerable
divergence.

### Rejected: window the plugin's list by the same rule

`visible_window` could be taught the native pane's sticky-offset behaviour. Refused
because it is the wrong direction: `visible_window` is the rule **every** plugin list
scrolls by and three native panes window with, so changing it to match one pane changes
every pane. ADR-39 already recorded the same hazard from the other side (the file
viewer's handover must relocate that function, not redefine it).

### Rejected: move the native pane off its list widget now, as groundwork

This is the honest closure, and it is refused *here* rather than refused outright. Four
behaviours come off `ListState` and each has a consumer that is not the paint: the
indicators are border chrome, the hitboxes are the click registry, the placeholder is
`App::pending_spawn`. Doing that in a change that also re-verdicts a table is how a
regression in primary navigation ships. It is the first item of the ordering below.

## 3. Why the three closed rows close on a conjunction

`no-active-session-write`'s probe is `!a_view_write_binding_exists(root)` — `true` means
"still missing". Marking the row `blocked: false` with that probe would require a view
write to **exist**, which is the opposite of what closing it means.

ADR-54 hit this first and its shape is copied: the row closes when

- the route exists (`key_context` is declarable, maps to this pane's focus, and the
  kernel still resolves that focus to this context), **and**
- the power the row named is still absent.

The second conjunct is the load-bearing half. Without it, a later change that granted a
view write would leave this row reading `closed` for the wrong reason, and the record
that the grant was *unnecessary* would be gone.

## 4. Why the two divergences become rows rather than staying in the oracle

`tests/bundled_session_list.rs` documents three enumerated divergences in `///` blocks
and asserts each with an `assert_ne!`. That is a good *measurement*: it fails if the
divergence closes, which forces a revisit.

What it cannot do is answer "may this pane be handed over". It is a port's file, it is
scoped to what the port claims, and a reader auditing the handover has no reason to open
it. The repo's rule — a verdict written in prose expires without telling anyone — applies
to a doc comment as much as to markdown.

So: the divergence tests stay where they are and keep their `assert_ne!`; the gate gains
a row per divergence with its own probe. Both can fail, and they fail for different
reasons — the oracle when the divergence closes, the gate when the *tree* stops matching
the recorded verdict.

The empty-state divergence is already a gate row (`no-centred-line`), which is the
precedent: one of the three was promoted when the gate was written and the other two were
not, for no recorded reason.

## 5. The wrap, and why it needs a test rather than a row

The brief that produced this work asked where the left column's circular wrap should
live once both its panes are plugins. ADR-56 answered it: nowhere new. The wrap moves
`self.focus` between `InputFocus::SessionList` and `InputFocus::Automations`, and a
handed-over pane is focused *as* the kernel's pane of that name — so both ends are kernel
focuses whoever draws either pane.

It is therefore **not** a blocker, and the risk is that a future reader re-derives it as
one (the automations gate had it as a row for a year of changes, and that row was about
the *plugin-keys* route). A row recording "closed" would be misleading — it was never
this pane's requirement. A test asserting the two facts that make it a non-issue is the
right shape: both ends are kernel focuses, and the condition is already "a pane provides
that list" rather than a feature flag.

## 6. The ordering, asserted

1. **The window.** It decides what a windowing seam looks like, and three of the other
   rows are functions of it (the indicators, the hitboxes, the placeholder).
2. **The module.** `resolve_rows` is what feeds both panes, so its destination depends
   on (1). The rest of the relocation — `compute_session_order`, `move_in_order`,
   `sort_alphabetically_within_groups`, `SessionMatch` — is navigation, reordering,
   sorting and search, and `session` is where the pure ones belong.
3. **The chrome and the two vocabulary rows.** Centring is additive and independent;
   the border chrome and the pending row are both downstream of (1).

Asserted in the gate rather than described here, so a change that closes (3) first
without closing (1) fails with the reason attached.
