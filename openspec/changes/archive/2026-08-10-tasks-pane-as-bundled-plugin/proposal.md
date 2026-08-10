# Render the tasks pane from a bundled Luau plugin

## Why

Phase 4 of the v2 migration turns thurbox's native panes into bundled plugins,
easiest first. The info panel went first and `docs/PHASE4-PANE-READINESS.md` §6
records the finding that came out of it: the info panel was *not* the cheap one —
it has the most kernel state and the most geometry per line in thurbox — and
**the tasks pane would have been the cheaper first port**, needing §2 (a kernel
reader) and §5 (a seat on screen), neither §3 nor §4. Both of those are now
closed, so the tasks pane is the next port and this change makes it.

The v1 behaviour being reproduced is `src/ui/tasks_panel.rs`: the toggleable
right-hand column that draws one row per task as `<glyph> <title>` with the
checkbox glyphs `☐`/`◐`/`☑`, the status colour, a trailing accent `⇄` on a task
that has an open related session, the selected row in the shared selected-item
style, non-matching rows dimmed while a global search is running with the matched
characters emphasised, and a muted `no tasks` line when the list is empty.

Today `src/app/view.rs` builds those rows out of `App` and hands them to a Rust
function on the UI thread. After this change a Luau plugin in its own VM, on the
plugin render worker, reads the same list through one capability-gated binding
and returns the same view tree.

Two things make this port worth doing rather than a second lap of the same track:

- **It is the first *list* pane.** Every remaining Phase 4 pane (automations,
  file viewer, global search, the session list) is a list of selectable rows with
  search emphasis, and none of the three styles a selectable row needs —
  selected, dimmed, match-emphasised — could be expressed by the view tree. The
  info panel has no selection and no search, so it never asked.
- **It is the first pane whose rows depend on the resolved geometry.** The native
  pane truncates each title to its column with an ellipsis, reserves room for the
  `⇄`, and scrolls the window to keep the selection visible. A plugin has no
  width and no height, so this port has to say which of those the kernel keeps
  and what the plugin's copy does instead — recorded, not hidden.

The native pane stays compiled in and stays the one on screen. Deleting it is
Phase 6 and `tests/teardown_gate.rs` guards that: this pane's row stays blocked
because `src/app/view.rs` still names `tasks_panel`.

## What Changes

- **A `tasks` capability and one reader.** `thurbox.tasks()` returns the task
  rows the kernel published, under `Capability::Tasks` — a fifth kernel-state
  capability alongside `sessions` / `metrics` / `automations`, for the same
  reason there are three of those and not one: the capability list is the
  install prompt, and "reads your task list" is a different question from "reads
  your sessions".
- **A `tasks` section on the published snapshot.** `PaneContext` gains
  `tasks: TasksSnapshot` — one entry per task row with its title, its status wire
  name, and the three view facts the kernel owns because it owns the keyboard and
  the search: which row is selected, which rows a running search dimmed, and
  which matched characters it matched. The section is bounded
  (`MAX_TASK_ROWS`) so a thousand-task list cannot exceed the view tree's node
  budget, and it is empty when the `tasks` feature is off, mirroring how the
  automations section already respects its feature flag.
- **Two emphasis flags on the view tree**, `dim` and `underline`, joining `bold`
  on `TextStyle` and on a `text` node. They are what a selectable row needs: the
  shared row-base style dims a row a search filtered out, and a matched run is
  accent + bold + **underlined**. Without them a list pane cannot be reproduced
  at all, in any pane, by anyone.
- **The native pane draws its view tree.** `ui::tasks_panel::tasks_tree` becomes
  the pane's rendering IR the way `info_tree` is the info panel's, painted by the
  shared `ui::plugin_pane::render_tree`. `visible_rows` stays beside it as the
  geometry step — window, fit, reserve — so the rows the tree is built from are
  the rows the pane draws, and the click hitboxes still come from the same
  window.
- **One segmentation of a fuzzy match, not two.** `ui::highlight` grows
  `highlight_runs`, the run-splitting its span builders already did, so the
  view-tree rows and the ratatui spans elsewhere cannot disagree about where a
  highlighted run starts.
- **A bundled `tasks` plugin**, shipped inside the binary next to `hello` and
  `info-panel`, `default_visible = false` so no user's layout changes. It maps
  status to glyph and token itself, applies the same selected > dimmed > status
  precedence, and re-implements the UTF-8-aware match segmentation in Luau — and
  `tests/bundled_tasks_panel.rs` asserts its tree **equals** the native pane's
  across content variants.
- **The two divergences are pinned by tests, not absorbed.** With a column too
  narrow for a title, or a list taller than the pane, the kernel's rows differ
  from the plugin's: the native pane ellipsizes and scrolls, the plugin's copy
  clips. Each is asserted, with the node that would close it named.

## Capabilities

- `plugin-host/kernel-state` — ADDED: the task list as a published section, its
  reader, what the kernel resolves (selection, search verdicts, linkage) and what
  it leaves to the pane (glyph, colour, layout); the bound on how many rows it
  publishes.
- `plugin-host/capabilities` — ADDED: reading the task list is its own declared
  capability.
- `plugin-host/view-tree` — ADDED: a text run may declare dim and underline
  emphasis, and the closed style vocabulary still admits no colour.
- `migration/phase-4` — ADDED: the second ported pane, and the rule that a pane
  whose rows depend on resolved geometry keeps that geometry in the kernel and
  names what its plugin copy does instead.

## Non-goals

- **Deleting or unwiring the native tasks pane.** It is what users see; the
  plugin is additive and hidden by default. Handover is Phase 6.
- **Keys in the plugin's pane.** The native pane's `j`/`k`/`e`/`r`/`n`/`o`/`d`
  act on thurbox's task list; a plugin pane would need the `input` capability and
  a way to *act*, which is not the same question as whether it can draw. The
  plugin's copy is read-only, and it deliberately does not draw the focused
  action footer — hint keys for actions the pane cannot perform would be a lie.
- **Closing the geometry gap.** An ellipsizing clip, a flush-right run, and a
  selection-windowed list are three separate node decisions; naming them from one
  pane's needs is how the gauge node got designed and the same discipline applies
  here. They are recorded as open with the measurement.
- **A `thurbox.format.*` helper table** (PHASE4 §7). This pane formats nothing,
  so it produces evidence about that decision rather than pre-empting it.
- **Porting a third pane.** Exactly one, completely.

## Impact

- New code: `src/plugin/bundled/tasks/{plugin.toml,init.luau}`,
  `tests/bundled_tasks_panel.rs`.
- Changed: `src/session/{pane_context.rs,view_tree.rs,plugin_manifest.rs}`,
  `src/plugin/{capabilities.rs,discovery.rs,kernel_state.rs,view.rs}`,
  `src/plugin/bundled/thurbox.d.luau`, `src/ui/{tasks_panel.rs,highlight.rs,plugin_pane.rs}`,
  `src/app/{mod.rs,view.rs}`, `docs/{PHASE4-PANE-READINESS.md,ARCHITECTURE.md}`,
  `CLAUDE.md`.
- Feature gate: everything under `src/plugin/` stays behind
  `#[cfg(feature = "plugins")]`; the snapshot section, the emphasis flags and the
  pane's tree builder are ungated kernel code, like `session::view_tree` and
  `session::pane_context` already are. `tests/bundled_tasks_panel.rs` is
  `#![cfg(feature = "plugins")]`. `cargo tree --edges normal | grep -c mlua`
  stays 0.
- Architecture: no new edge. The snapshot section is pure data in `session`; the
  Lua conversion is `plugin → session`; the tree builder is `ui → session`; `app`
  already reaches all three. `tests/architecture_rules.rs` is unchanged, and the
  one place that must see both `ui::tasks_panel` and `plugin::PluginHost` is an
  integration test, which is not in the library's module graph.
- Snapshots: none move. No pinned frame contains the tasks pane, and the pane's
  own rendering is unchanged — which the retained cell-level assertions in
  `src/ui/tasks_panel.rs` are what check.
