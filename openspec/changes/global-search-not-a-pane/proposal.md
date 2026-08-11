# Record global search as structurally unportable to a plugin pane

## Why

Phase 4 turns thurbox's native panes into bundled plugins, easiest first: the
info panel (ADR-27), the tasks pane (ADR-29), the file viewer's tree (ADR-30).
Each port answered the phase's real question — *did the plugin API suffice, or
did it have to be widened* — and each answer was a widening: eleven style
tokens and a gauge node, a kernel-state snapshot, two emphases, a selected list
and a selection role.

Global search is the next surface on that list, and the answer is different in
kind. **It is not a pane.** The v1 behaviour is `Ctrl+/`
(`src/ui/global_search.rs` + `src/app/search.rs`): a non-modal strip docked
full-width above the footer that searches four scopes at once, highlights the
characters it matched **inside the session list, the tasks panel and the
automations pane**, moves those panes' cursors as a live preview while focus
stays in the query box, jumps focus into the owning pane on `Enter`, and
restores every one of them from a snapshot on `Esc`.

Three of those five behaviours are reaches *outside* a rectangle. A plugin pane
owns a rect, its own keys, and nothing else — which is not a shortcoming of the
node catalogue but the shape of ADR-V1's kernel/plugin split. So this change
produces the **record** rather than the plugin: the blockers, each derived from
the source, separated into the ones a wider vocabulary would close and the ones
it would not.

What makes this worth a change of its own rather than a paragraph is that a
plugin reproducing *part* of the strip was available and was rejected. A pane
fed a published `search` section could have drawn the result list in the right
column, and the teardown gate's `global-search-plugin` row would have looked
one step closer to ready. It would have been a pane that cannot search, cannot
highlight, cannot preview and cannot jump — reporting a capability the host does
not have, which is the one outcome Phase 4's own spec forbids: *a gap worked
around by a shortcut a third party could not take MUST be recorded as still
open*.

## What Changes

- **No bundled `global-search` plugin is shipped.** The surface is recorded as
  structurally unportable instead, with the blockers named individually.
- **Nothing in the host is widened.** No capability, no view node, no style
  token, no pane slot, no binding. Every gap this port found is recorded; none
  is closed speculatively, because closing the cheapest of them (a bottom slot)
  would buy layout capacity whose only occupant still could not work.
- **The verdict is machine-checked**, not merely written down. A new
  `tests/global_search_pane_gap.rs` re-derives each blocker from the tree the
  way `tests/teardown_gate.rs` re-derives its verdicts, so closing one fails the
  record and names it. It also asserts the other half: that no bundled plugin
  claims the surface, which is what keeps the record and the teardown gate from
  disagreeing.
- **`docs/PHASE4-PANE-READINESS.md` gains the port's section**: the blocker
  table, split into *structural* (four rows, none closable by a node) and
  *vocabulary* (four rows, all closable), plus the finding about what shape
  would make the surface reachable at all.
- **The native strip is untouched**, and so is the teardown inventory: the
  `global-search-plugin` row stays blocked, now for a recorded reason rather
  than for want of a directory.

The blockers, in the order they stop the port:

| # | What the strip needs | Where the host stands |
|---|---|---|
| 1 | a full-width band above the footer | `PaneSlot` is a closed set whose only member is `Right`; the strip's band is `RegionId::GlobalSearch`, and `LayoutParams::right_regions` is the only place a plugin pane is seated |
| 2 | the query and its results | no capability publishes either, and none could without the kernel computing the search: the session scope reads every session's live vt100 screen (`App::session_content_match`), which is the widest read in the application |
| 3 | **producing** the restyling of rows in other panes | the verdict already crosses *outward* — a published task row carries `dimmed` and `match_positions` — but each pane applies it to *its own* rows (`ui::highlight` in `project_list`, `tasks_panel`, `automations_panel`), a plugin's tree is painted with no access to any pane but its own, and nothing carries a query back the other way |
| 4 | move another pane's cursor, take focus, restore a snapshot | the kernel-state channel is read-only by construction — every binding under `Capability::{Sessions,Metrics,Automations,Tasks,Files}` reads a published snapshot and nothing writes back |
| 5 | a framed block titled ` Search ` | no frame node, and a pane's own frame is drawn by the host in the border/focus style |
| 6 | a bottom-anchored hint row under a `Min(0)` result list | `ViewNode::Column` stacks children at their natural height from the top; already recorded as missing by the file-viewer port |
| 7 | the search accent the frame, prompt and caret use | `StyleToken` names no `search_bar` role, and a plugin may name no colour |
| 8 | an italic snippet line | `TextStyle` carries `bold`, `dim`, `underline`, `selected` — no italic |

Rows 1–4 are structural: each is a power the pane model withholds on purpose,
and adding all four amounts to a different concept, not a wider catalogue. Rows
5–8 are vocabulary, closable in an afternoon, and worth nothing on their own.

## Capabilities

- `migration/phase-4` (modified) — adds what a port must produce when its
  surface cannot be a pane at all, records global search as that case, and
  requires the verdict to be re-derived from the source rather than asserted in
  prose.

## Non-goals

- **No `PaneSlot::Bottom`.** It is the cheapest blocker and closes none of the
  other three; a band whose only occupant cannot search is capacity built for
  nothing. It should land with the surface that needs it.
- **No write channel into kernel view state** (set focus, move another pane's
  cursor). That is blocker 4, and granting it would let any installed plugin
  move the user's cursor and take focus — a change to what a plugin *is*, not a
  binding to add in passing.
- **No cross-pane decoration channel** (a plugin-published query the native
  panes consult). See `design.md` §3: it inverts the dependency Phase 4 exists
  to keep pointing one way.
- **No italic, frame node or `search_bar` token added speculatively.** Recorded
  as open, closable by whoever needs them.
- **No design of the provider shape.** `design.md` §5 names it as the shape that
  would make the surface reachable, and deliberately stops there: it should be
  designed from the code-review and session-list ports too, which are the other
  two surfaces in this phase that are not simple panes.
- **No change to the native strip**, its keys, or its rendering — including no
  refactor of it into a view tree, which would be the first step of a port that
  is not happening.

## Impact

- New: `openspec/changes/global-search-not-a-pane/`,
  `tests/global_search_pane_gap.rs`.
- Modified: `docs/PHASE4-PANE-READINESS.md` (the port's section),
  `docs/PHASE6-TEARDOWN-READINESS.md` (the pane table's global-search row points
  at the reason).
- Unchanged, deliberately: `src/` in its entirety, `tests/teardown_gate.rs`, and
  every insta snapshot.
