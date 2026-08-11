# A run may yield its width to its siblings, and the kernel ellipsizes it

## Why

Three of thurbox's own list panes fit one run of each row to the column and the
plugin catalogue cannot say it. The tasks pane reserves the trailing `⇄` marker's
room and ellipsizes the title into what is left (`ui::tasks_panel::task_rows`); the
automations pane fits a name against `width − prefix − tail`; the session list does
the same for a session name. A plugin has no width — deliberately, five times over
(ADR-26, ADR-29, ADR-30, ADR-31, ADR-39) — so its copy draws the whole title and the
renderer clips it at the pane edge.

The consequence is not cosmetic. On a 20%-wide column a long title loses its `…`
**and** the marker after it: the two panes show *different information*, which
`tests/bundled_tasks_panel.rs` enumerates as its last remaining divergence and
`tests/tasks_pane_input_gap.rs` records as its one vocabulary row. The automations
gate's `no-fitted-name` says the same thing, and names the same closure — *"an
ellipsizing clip plus a flush-right run"*, unchanged since ADR-29. The flush-right
half already landed (`ViewNode::Fill`). This is the other half, and it is the last
thing standing between the tasks pane's plugin and a tree that is equal at **every**
width rather than at wide ones.

## What Changes

- **`TextStyle` gains `ellipsize`.** A run that declares it *yields its width to its
  siblings*: the kernel gives every other run on the line its intrinsic width, hands
  what is left to the ellipsizing runs, and truncates them with `…`. Consecutive
  ellipsizing runs share **one** budget, because a title split into matched and
  unmatched runs is one string to a reader.
- **It is a style field, not a node.** The gates that asked for this predicted the
  shape (`tests/automations_pane_handover_gap.rs` probes `TextStyle` for the flag and
  `ViewNode` for a `Clip`/`Ellipsis` kind), and a field is what does not multiply:
  `ViewNode` gains no variant, so the recorder, the motion walk, `is_inlineable`,
  `height_of` and every `match` are untouched.
- **The kernel fits it with `ui::truncate_ellipsis` — the function the native panes
  fit with.** One implementation, two callers, which is the arrangement ADR-30 chose
  for the scroll window and ADR-39 for the scroll track, and for the same reason:
  nothing else forces them to agree.
- **The native tasks pane declares it instead of fitting.** `task_rows` loses its
  `width` argument and its `truncate_ellipsis` call; `tasks_tree` marks the title
  runs `ellipsize`. So the pane is fitted by the same code that fits the plugin's
  copy, and `ui::tasks_panel` consults **no** dimension at all.
- **The bundled `tasks` plugin declares it too**, and its enumerated divergence
  retires: `tests/bundled_tasks_panel.rs` now asserts the two trees are equal at a
  **narrow** width, where they used to be asserted to differ.
- **The recordings are regenerated from the native builder**, which is what ADR-42
  requires and permits: the native pane's tree genuinely changed (the fit moved out
  of it), the builder is still here to record, and the diff is one word per title
  run. It is not a re-record from the plugin, which is the thing that rule forbids.

## Non-goals

- **The automations pane and the session list do not adopt it here.** The
  vocabulary is what was missing; adopting it is one line per pane plus that pane's
  own re-recording, and each belongs to the change that hands that pane over.
  `no-fitted-name` therefore stays blocked, with its probe narrowed to the reason
  that is actually left — `resolve_rows` still fits the name itself — rather than to
  "the catalogue cannot say it", which is no longer true.
- **No horizontal scroll, no wrap.** A line clips; that is what distinguishes it
  from `ViewNode::Paragraph`. This changes *what* is clipped, not whether.
- **No width is reported to a plugin.** The whole point: the plugin declares which
  run gives way, the kernel resolves the columns.
- **Display width versus characters is not fixed here.** `truncate_ellipsis` counts
  **characters**, so a run of double-width glyphs can still exceed its cell budget —
  which is exactly what the native panes do today. Matching them is the requirement;
  changing the rule would make the plugin's copy differ from the pane it reproduces.
  Recorded as a known corner rather than silently diverged.

## Impact

- Affected specs: `plugin-host/view-tree` (one ADDED requirement),
  `plugin-host/authoring` (one MODIFIED).
- Affected code: `src/session/view_tree.rs`, `src/plugin/view.rs`,
  `src/plugin/capabilities.rs`, `src/ui/plugin_pane.rs`, `src/ui/tasks_panel.rs`,
  `src/app/view.rs`, `src/plugin/bundled/tasks/init.luau`,
  `src/plugin/bundled/thurbox.d.luau`, `tests/view_tree_record/mod.rs`,
  `tests/bundled_tasks_panel.rs`, `tests/snapshots/bundled_tasks_panel__*.snap`,
  `tests/tasks_pane_input_gap.rs`, `tests/automations_pane_handover_gap.rs`.
- Docs: `docs/ARCHITECTURE.md` (ADR-52), `docs/PHASE4-PANE-READINESS.md` §27,
  `CLAUDE.md`.
- No schema change, no settings change, no new dependency, no capability.
