# Give a plugin pane every seat a native pane occupies

## Why

`PaneSlot` has exactly one member, `Right`. So a plugin pane is placeable **as a
pane** and not placeable **where any of thurbox's own panes are**: the session
list *is* the left column, the automations pane is the band beneath it, the info
panel is its own `Percent(15)` column left of centre, and the code review owns the
central pane. A reproduction of one of those draws its content correctly into a
different rect with a different title, which is why six handover attempts across
five gate files record the same row — `no-left-seat`, `no-central-seat`, twice each
— and why `docs/PHASE4-PANE-READINESS.md` §14 lists "the same seat" first among the
five requirements a handover has.

v1's behaviour being extended: the layout already seats *N* plugin panes, but only
in the right column (ADR-24, ADR-28). Every other seat is reachable by the kernel's
own renderers alone, addressed by a `RegionId` the workspace tree already places —
`SessionList`, `Automations`, `Info`, `Center`. Nothing in the tree needs
inventing; what is missing is a way for a **manifest** to name one of those seats,
and a rule for what happens when a plugin pane and the kernel's own pane both want
it.

The blocker is worth closing on its own, before any pane moves, because it is the
one requirement in §14's table that is shared by five of the six remaining
handovers. Closing it per pane would be closing it five times.

## What Changes

- **`PaneSlot` gains four seats**: `left` (the left column, where the session list
  is), `left-bottom` (the band beneath it, where the automations pane is),
  `center-left` (the narrow column left of centre, where the info panel is) and
  `center` (the central pane the agent terminal, the shell and the review share).
  `right` stays the default, so every existing manifest is unchanged.
- **A slot names a region.** `PaneSlot::seat()` maps each of the four to the
  `RegionId` the workspace tree already places, and `right` to `None` — it is a
  *column* that seats any number of panes, each in its own `RegionId::Plugin(i)`.
  One table, which is also what the gate probes read.
- **A visible plugin pane takes its seat, and the kernel's own pane for that seat
  is not drawn.** That is what a handover *is*; the alternative (the native pane
  wins) would make every new seat unreachable until the renderer it exists to
  replace is deleted, so the mechanism could never be exercised before the change
  that depends on it. Hiding the plugin pane restores the native pane, so the
  takeover is reversible in both directions.
- **A claimed seat is placed.** A seat whose kernel pane is toggled off is still
  carved when a plugin pane claims it, so a pane in `center-left` appears whether
  or not the user has the info panel open. Without a claim the layout is
  bit-identical: every existing geometry test and snapshot is unchanged.
- **The lower-left band's height policy covers a plugin pane.** That band is the
  one place in thurbox where a pane's height is a function of its own content
  (`(count + 2).clamp(3, 10)`). A plugin is never told its size, so the kernel
  keeps the policy and reads the count off the pane's tree —
  `ViewNode::stacked_row_count`, the number of rows its outermost stack contains.
- **Two panes for one seat is decided, not undefined**: the first in publication
  order takes it and the rest are not placed — the rule the right column already
  applies when it runs out of columns.
- **The three reproductions whose native seat is not the right column move into
  it**: `session-list` → `left`, `automations` → `left-bottom`, `info-panel` →
  `center-left`. All three stay seeded hidden, so no fresh install's screen
  changes; showing one now compares the two panes in the *same* rect instead of in
  two different ones, which is what `tests/bundled_automations_panel.rs`'s
  placement divergence asked for.

## Capabilities

### Modified Capabilities

- `plugin-host/manifest`: the slot vocabulary is five seats rather than one, and
  each names a region the kernel already places.
- `plugin-host/panes`: what happens when a plugin pane and a kernel pane want one
  seat, what a claimed seat does to the layout, and how the one content-derived
  band sizes a plugin pane.
- `layout/slots`: a seat may be occupied by a plugin pane instead of the kernel's
  own, and a claim is enough to carve it.

## Non-goals

- **No pane is deleted, and no native renderer stops being reachable.** Every one
  of the six is still drawn whenever no plugin pane has taken its seat, which for
  a fresh install is always — every bundled pane seeds hidden.
- **No new region, and no geometry change.** The four seats are existing
  `RegionId`s with their existing shares and width rules. `compute_layout` gains
  no branch.
- **The central pane's chrome does not follow.** A plugin pane seated in `center`
  draws its own titled block; the tab strip (`Agent · Review · Shell`) and the F9
  collapse chevron are kernel chrome for the kernel's central views and are not
  drawn over it. That is a real gap for the code review's handover and is recorded
  as one rather than papered over.
- **Focus is unchanged.** A seated pane is still focused as
  `InputFocus::PluginPane`, so the scoped keyboards (`KeyContext::SessionList`,
  `Automations`, `Tasks`) still do not resolve for it. That is the *other*
  blocker in every handover gate and it stays open.
- **No band slot.** Global search's full-width strip above the footer
  (`RegionId::GlobalSearch`) is not a seat: it is excluded from the handover by
  §10's verdict, and `tests/global_search_pane_gap.rs` keeps its row blocked.
- **No file-viewer or tasks seat.** Those two native panes are right-column
  occupants, and `right` already seats a plugin pane in that column. The review's
  changed-files list wants `RegionId::FileViewer` *specifically* (the column is
  forced present while a review is open), and that row stays blocked.
- **No manifest binding to an action or a feature flag.** §14's second row — a
  pane answering `F2`, a pane gated by `[features] info_panel` — is the next
  change.

## Impact

The plugin host ships in every install since Stage B (ADR-40), so this is not
behind a Cargo feature; `--no-default-features` has no plugin panes and therefore
no seat claims, and its layout is untouched.

`src/session/plugin_manifest.rs` (`PaneSlot`, `seat()`),
`src/session/view_tree.rs` (`stacked_row_count`), `src/app/mod.rs` (`plugin_seat`,
`seat_taken`, `layout_for`), `src/app/view.rs` (paint a seated pane, skip the
native occupant), `src/ui/layout.rs` (doc only — the seat's occupant is no longer
fixed), `src/plugin/bundled/{session-list,automations,info-panel}/plugin.toml`,
`src/plugin/bundled/thurbox.d.luau`, `src/app/acceptance.rs`,
`tests/{automations,session_list,code_review}_pane_handover_gap.rs`,
`tests/tasks_pane_input_gap.rs`, `tests/global_search_pane_gap.rs`,
`tests/bundled_automations_panel.rs`, `docs/ARCHITECTURE.md` (ADR-46),
`docs/PHASE4-PANE-READINESS.md`, `CLAUDE.md`.
