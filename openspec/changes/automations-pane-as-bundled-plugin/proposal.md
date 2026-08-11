# Render the automations pane from a bundled Luau plugin

## Why

Phase 4 turns thurbox's native panes into bundled plugins, easiest first: info
panel (ADR-27), tasks (ADR-29), file viewer (ADR-30). The automations pane is
next, and it is the first port that is not simply "another list":

- it is the only pane in the phase that lives in the **left column, beneath the
  session list** — every pane ported so far sat in the right column, which is the
  only column `PaneSlot` can name;
- its rows carry a **composed summary** (`daily 09:00 · spawn · in 3h`) built from
  a schedule, an action and a countdown, so it is the first port that has to
  decide whether the kernel publishes a display string or its parts;
- and it is the first list pane whose **scroll anchor and cursor appearance come
  apart**: it windows to the cursor's row whether or not the pane is focused, but
  draws the cursor only when it is (or when a global search previews it).

The v1 behaviour being reproduced is the body of `src/ui/automations_panel.rs`:
one row per automation as `<space><marker><space><name> — <summary><space>`, where
the marker is `●` for an enabled automation and `○` for a disabled one, the name
is drawn in the theme's secondary text when enabled and muted when disabled, the
row the cursor is on takes the shared selected-item appearance (accent + bold), a
row a running global search filtered out is muted and dim, characters the search
matched are accent + bold + underlined over whatever the row's base was, the
summary tail is drawn in the row's base style, and an empty pane shows a muted
`none` — or `none — Ctrl+N to add` while the pane is focused. The list windows so
the cursor's row stays visible.

Today `src/app/view.rs` builds those rows on the UI thread and
`render_automations_pane` assembles ratatui spans. After this change a Luau plugin
in its own VM, on the plugin render worker, reads the same rows through the
**existing** `automations` capability and returns the same view tree.

The native pane stays compiled in and stays the one on screen. Handover is Phase 6
and `tests/teardown_gate.rs` keeps this pane's row blocked while `src/app/view.rs`
still names `automations_panel`.

## What Changes

- **The `automations` capability gains a second reader.** It already grants
  `upcomingAutomations()` — the info panel's *filtered, countdown-resolved* view of
  what is due. The pane needs the whole list, enabled and disabled, with the
  search's verdict and the cursor. Both are the same question to ask a user
  ("reads the automations you have scheduled"), so this is one more reader behind
  the capability that exists rather than a new capability.
- **A new published section: the automations pane's rows.** `PaneContext` gains
  `automations: AutomationsSnapshot` (the existing `Vec` becomes
  `upcoming_automations`, matching the reader that reads it). Each row carries the
  automation's name, its **resolved schedule label**, its action's wire name,
  whether it is enabled, the seconds until it is due, the search's verdict, and the
  matched byte offsets. The section carries the cursor's row, whether that cursor
  is drawn, and whether the pane is focused. Bounded by `MAX_AUTOMATION_ROWS`, and
  empty when the `automations` feature is off — mirroring the task and file
  sections.
- **The summary is published as parts, not as a string.** The kernel resolves what
  a sandboxed plugin cannot: the cron expression's human label (thurbox's own
  mapping, shared with the automation editor) and the countdown in seconds (a VM
  has no clock). The plugin composes `<schedule> · <action> · <when>` itself,
  including the `disabled` / countdown / `—` precedence. `design.md` §2 records
  why publishing the finished string was rejected.
- **The anchor and the appearance are published separately.** The section names the
  cursor's row *and* whether it is drawn, because this pane windows to the cursor
  while unfocused but does not highlight it. That is the second, independent case
  for ADR-30's rule that a list's selected row is an **anchor** and a run's
  selected style is an **appearance**.
- **The native pane draws its view tree.** `ui::automations_panel::automations_tree`
  becomes the pane's rendering IR, painted by the shared
  `ui::plugin_pane::render_tree`; `resolve_rows` keeps the one width-dependent step
  (fitting a name to the column, with the marker and the summary's room reserved).
  The window moves out of the pane and into the renderer, resolved from the
  anchor — so this pane, like the file viewer, is compared frame-for-frame at a
  height that scrolls. The pre-port span renderer is retained as a `#[cfg(test)]`
  oracle.
- **`format_countdown` moves to `ui`.** The composition rule has to be one
  function, reachable from the pane's tree builder, the automations list modal and
  the test; the countdown is presentation, and `ui` is where thurbox's other
  display formatters live.
- **A bundled `automations` plugin**, shipped in the binary beside `hello`,
  `info-panel`, `tasks` and `file-viewer`, `default_visible = false`. It owns the
  markers, the colour roles, the emphasis precedence, the summary composition and
  the empty-state line; `tests/bundled_automations_panel.rs` asserts its tree
  **equals** the native pane's across content variants and **paints the same
  frame** when the pane scrolls.
- **The left-column finding is recorded and pinned by a test.** `PaneSlot` names
  one slot, `right`, so the reproduction cannot be placed where the native pane
  sits. This change does **not** widen it (see Non-goals); it states the cost, and
  a test asserts a manifest naming `left` is still refused, so the finding cannot
  quietly go stale.

## Capabilities

- `plugin-host/kernel-state` — ADDED: the automations pane's rows as a published
  section; a row's composed display string is published as its parts; a list
  section's scroll anchor and drawn cursor are separate facts; the section's bound
  and feature gate.
- `plugin-host/capabilities` — ADDED: reading scheduled automations is one
  capability covering both the upcoming list and the pane's rows.
- `migration/phase-4` — ADDED: a fourth native pane is reproduced; a port states
  when the reproduction cannot be placed where the native pane sits, and pins it.

## Non-goals

- **A `left` pane slot.** The native pane sits beneath the session list, in a
  region whose height is a function of the automation count
  (`AUTOMATIONS_PANE_MIN_ROWS`..`AUTOMATIONS_PANE_MAX_ROWS`), and `PaneSlot` names
  only `right`. Placing a plugin pane there needs four things this change
  deliberately does not do: a second slot in the manifest vocabulary, a plugin
  region inside `left_column`, a `RegionId::Plugin(i)` index space that no longer
  assumes one contiguous run of right-column panes, and a **height policy** for a
  left plugin pane — the native pane's height comes from its row count, which the
  kernel does not know for a plugin pane until it has a tree. That is a layout
  change every user sees, in the file that owns the geometry of every pane, and it
  is larger than this port. It is recorded in `docs/PHASE4-PANE-READINESS.md` §10
  with its cost, and pinned by a test.
- **The pane's keys.** `j`/`k`/`Space`/`r`/`d`/`n`/`e` act on thurbox's own
  automations; the plugin's copy is read-only and draws no hints for actions it
  cannot perform.
- **The Ctrl+P automations list modal and the in-pane editor.** Different surfaces
  with their own state; only the pane is in scope.
- **The run-history list** (`src/ui/automation_detail.rs`). It renders in the
  *central* pane, not in the automations pane, and it is reached through a focus
  the plugin's copy cannot enter.
- **A `thurbox.format.*` helper table.** This port makes the case for one a second
  time — the countdown formatter the info-panel plugin carries is now needed
  verbatim by a second bundled plugin — but adding it here would change a shipped
  plugin in the same change that measures the need. `design.md` §5 records the
  measurement; the next port should decide it.
- **Deleting or unwiring the native pane.** Phase 6.
- **Porting a fifth pane.** Exactly one, completely.

## Impact

- New code: `src/plugin/bundled/automations/{plugin.toml,init.luau}`,
  `tests/bundled_automations_panel.rs`.
- Changed: `src/session/{pane_context.rs,plugin_manifest.rs}`,
  `src/plugin/{capabilities.rs,discovery.rs,kernel_state.rs}`,
  `src/plugin/bundled/thurbox.d.luau`,
  `src/ui/{mod.rs,automations_panel.rs}`,
  `src/app/{mod.rs,view.rs,automation.rs,acceptance.rs}`,
  `docs/{PHASE4-PANE-READINESS.md,PHASE6-TEARDOWN-READINESS.md,ARCHITECTURE.md}`,
  `CLAUDE.md`.
- Feature gate: everything under `src/plugin/` stays behind
  `#[cfg(feature = "plugins")]`; the snapshot section and the pane's tree builder
  are ungated kernel code, as `session::pane_context` and `session::view_tree`
  already are. `cargo tree --edges normal | grep -c mlua` stays 0.
- Architecture: no new edge. The section is pure data in `session`, the Lua
  conversion is `plugin → session`, the tree builder is `ui → session`, and
  `app` calls `ui` as it already does. `tests/architecture_rules.rs` is unchanged;
  the one place that must see both `ui::automations_panel` and
  `plugin::PluginHost` is an integration test, outside the library's module graph.
- Snapshots: none move. Every pinned frame contains the empty automations pane's
  `none`, and the retained cell-level differential against the pre-port span
  renderer is what checks that the rendering is unchanged.
