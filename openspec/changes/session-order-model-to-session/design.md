# Design

## The question this change answers

Not "can the session list be a plugin" — that is
`tests/session_list_pane_handover_gap.rs`'s question and the answer is still no. This
change answers a smaller one that the handover happens to depend on: **which layer owns
the order thurbox navigates its sessions in?**

Today the answer is `ui`. `App::switch_session_forward`, `App::move_active_session` and
`App::sort_sessions_alphabetically` — three kernel behaviours bound to `Ctrl+J`, `Shift+J`
and `Shift+S` — reach into `crate::ui::project_list` for their comparator, their reorder
and their sort. The `ui` module is documented as "pure rendering functions"; the
architecture rules let `ui` see `app`, and not the reverse, precisely so that rendering
cannot become a dependency of the model. Here it is one.

## Where the model goes, and why `session` rather than `app`

`session` is the crate's dependency sink: pure data, no crate-internal references, no
effects. Everything moving is a pure function of `&[&SessionInfo]` plus a cursor —
`SessionInfo` is itself a `session` type — so the move is to the layer that already owns
the value.

This is the opposite call from the file viewer's (ADR-58), where `FileViewerState` went to
`app` and **not** to `session` despite `session::review` being the obvious parallel. The
rule `migration/handover` records from that case is the one applied here, and it selects
differently because the input differs: *a model that performs side effects must not be
relocated into a layer the architecture keeps free of them, however well its types would
fit*. `FileViewerState` calls `read_dir`. Nothing moving here reads, writes, spawns or
blocks: `compute_session_order` sorts, `move_in_order` swaps index ranges,
`resolve_rows` copies fields out of `SessionInfo`. So the same rule that sent one model to
`app` sends this one to `session`.

The module is `session_list.rs`, named for the domain object as `task.rs`, `review.rs`,
`automation.rs` and `message.rs` are. Not `session_order.rs`: the order is the largest part
but not the whole — the matcher and the resolved row are the list's model too, and a name
that covered only the ordering would invite the next reader to file the next piece
somewhere else.

## Where the boundary falls

The cut is **geometry**. Everything that is a pure function of the session set moves;
everything that needs a resolved width, a ratatui type, or a theme stays.

| Stays in `ui::project_list` | Because |
|---|---|
| `resolve_items`, `fit_status_text`, `row_used_columns` | decide how much text fits a resolved column |
| `session_list_tree` and the node/style builders | the pane's drawing, which the plugin reproduces |
| `render_left_panel`, `render_session_section`, the indicators | ratatui |
| `pending_spawn_slot`, `PendingSpawnSlot` | fed by the window, whose seam is undecided |
| `legacy_session_line` and friends | the pre-port span oracle |

`SessionRow` moves even though it is documented as "the *view* row", because what makes it
a view row is that the search's verdict and the cursor have been folded in — both facts
about the model — and because `resolve_rows`, which the gate names, produces it. Its one
geometry-bearing field, `status_text`, is `None` on every row `resolve_rows` returns; it is
filled by `resolve_items`, which stays. So the type crosses the boundary and the field's
*producer* does not, which is the shape the split already had.

## Rejected alternatives

**Move the model inside the handover, as `migration/handover` says.** Rejected because the
handover is refused on `the-window-is-the-list-widgets`, which this change cannot close
(measured below). Under that rule the model would stay in the rendering layer until the
window becomes a seam — an indefinite wait for a defect that has nothing to do with the
window. The precedent for hoisting is in the same spec: a pane's *keyboard* is required to
become actions in a change **before** its handover, for the reason that applies verbatim
here — a commit that relocates a model and moves who draws a pane makes a behavioural
difference read equally as either.

**Re-export the moved items from `ui::project_list`,** so no caller changes. Rejected: the
re-export is the defect. A `pub use` would leave `App` still spelling its own navigation as
`crate::ui::…`, leave the gate's probe passing against a module that no longer holds the
code, and leave the handover with exactly the same deletion problem — the module would
still be what the kernel names.

**Move `pending_spawn_slot` too, since it is pure.** Rejected on the spec's own ordering
rule (`migration/phase-4`): the relocation of anything the widget's window feeds is
ordered after the windowing decision. The placeholder's index is an index into the
rendered rows, consumed by the widget's offset alongside the hitboxes and the indicators.
Purity is not the criterion; being downstream of an undecided seam is.

**Move the width fit as well, to leave `ui::project_list` purely a painter.** Rejected:
`session` may not hold geometry, and the fit exists only because the pane has a resolved
width. `migration/phase-4` already keeps a pane's geometry in the kernel's pane rather than
in a plugin or a data type.

**Take the opportunity to converge the fit onto `ellipsize`,** as the automations pane did
(ADR-55) — its own comment says this pane "owes" that adoption. Rejected here, not
forever: that is a change to what the pane *draws*, it would move the recorded oracle, and
`migration/handover`'s convergence rule wants it in a change whose whole content is the
convergence. Mixing it into a relocation would make the relocation unreviewable — the claim
"no function body was edited" is what makes this change checkable at a glance.

## Why the window row is not closed here, with the measurement

Recorded so that the next attempt starts from evidence rather than from the row's prose.
40 sessions across four repo groups, pane inner height 30, cursor walked down one row at a
time. `native` is `ListState::offset()` after the stateful render; `shared` is
`ui::visible_window` over the flat children a plugin pane declares, where each group header
is its own child.

| cursor | native item offset | shared child window |
|---|---|---|
| 0 | 0 | 0..30 |
| 3 | 0 | 1..31 |
| 5 | 0 | 3..33 |
| 10 | 0 | 9..39 |
| 20 | 0 | 14..44 |
| 28 | 1 | 14..44 |
| 39 | 12 | 14..44 |

The native pane does not scroll at all for the first 28 of 40 rows — the cursor walks down
the pane and the list holds still. The shared rule scrolls after the third keypress and is
pinned to the list's tail for the whole second half. Adopting the shared rule in the native
pane would therefore be a visible regression in the pane every user navigates with, and the
spec already refuses the converse (redefining the shared rule for one pane's widget).

Two further halves of the same row are unaffected by scroll policy, and are why the row is
not merely a policy choice:

- **Item granularity.** A group header travels with the row below it, so a two-line item is
  **one** hitbox and the window never splits a header from its row. A plugin's list emits
  the header as its own child.
- **Click index space.** `App::render_plugin_panes` maps a seated pane's hitbox index to a
  kernel row as `row(index - 1)`, which holds for every pane handed over so far because
  each emits one child per row. This pane's children include headers, so the mapping is
  wrong by the number of preceding headers and grows through the list. A handover today
  would ship a session list whose clicks select the wrong session.
