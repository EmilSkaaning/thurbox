# A line the kernel centres, for the one thing every node draws from the left

## Why

`tests/session_list_pane_handover_gap.rs` holds this row against the session list's
handover:

> `no-centred-line` — the empty state (`No sessions yet`, `Press Ctrl+N to create one`)
> is drawn **centred**. Every node draws from the left; `Fill` can push a run flush right
> and nothing centres. The one enumerated divergence of the port's oracle, and the plugin
> draws the same words left-aligned.

Small, and a handover would ship it: the session list is empty on a fresh install, so the
*first* frame a new user sees is the one this row is about.

It is also the last placement rule the catalog is missing. Left is what every node does;
flush-right has had a spelling since ADR-31 (`Fill` before a run, whose residue the kernel
resolves); centre has none, and cannot be built out of the parts. A plugin could put a
fill on either side of a run and get *approximately* centred — the residue is split evenly
between fills — but the odd column goes to the **left** fill, where ratatui's own
`Alignment::Center` puts it on the right. So the shape a pane would have to write is both
awkward and wrong by one column half the time, which is worse than not having it: it looks
like it works.

## What Changes

- **`ViewNode::Center`** — inline runs packed left to right on **one** row, that row
  placed centrally in the width the node is given. Admits exactly the children
  `ViewNode::Line` does, and clips the same way.
- **The kernel resolves the placement**, from the rect the plugin is never shown, by the
  same arithmetic ratatui centres a `Paragraph` with — so a pane centring a line and the
  native pane beside it cannot land in different columns.
- **`ui.center(children)`** joins the constructor loop that already builds `row`, `line`,
  `paragraph` and `column`, and the declared type surface describes it, so a bundled pane
  can use it under `luau-analyze`'s strict mode.
- **Nothing existing changes.** No node gains a field, no existing tree is edited, no
  bundled plugin is touched, and no recorded oracle or acceptance snapshot moves.

## Non-goals

- **A general alignment.** No `align` field, no `Alignment` enum, no right-aligned
  variant. Right already has a spelling and a second one would be two ways to say one
  thing; a `justify` has no consumer at all. The catalog's stated discipline is that it
  holds "the set thurbox's own panes need, not a general drawing API".
- **Turning `Line` into a struct variant** so it could carry the alignment. Rejected in
  `design.md` §2 with its cost.
- **Adopting it in the session list.** `ui::project_list::render_empty_sessions` still
  draws its centred `Paragraph` directly and the bundled plugin still emits two
  left-aligned rows, because changing either moves a recorded golden — which belongs to
  the handover, whose refusal this row is one line of. The row is therefore re-verdicted
  **met** on the same rule the gate already applies to `no-left-seat` and
  `no-central-seat`: a row closes when the *route* exists, not when the reproduction takes
  it. `the_empty_pane_is_the_one_place_the_plugin_differs` in
  `tests/bundled_session_list.rs` goes on asserting that the two panes still differ, so
  "the vocabulary exists" cannot come to read as "the panes agree".
- **A capability.** This adds a node kind and its constructor. No `Capability` variant, no
  new module binding, no widening of what a plugin may read or write.

## Gate

No compile-time gate: the node lives in `src/session/view_tree.rs` and its renderer in
`src/ui/`, both in every build. The Luau constructor is behind the `plugins` feature with
the rest of the host. Both builds are verified.
