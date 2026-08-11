# Render the session list from a bundled Luau plugin

## Why

ADR-V1 says everything but six things is a plugin, **including the session
list**. That clause is the whole architecture's load-bearing one: if the densest,
most-redrawn pane in the application cannot be a plugin, then the plugin API is a
second-class surface for decorations and nothing important dogfoods it.

`docs/SPIKE-SESSION-LIST.md` measured exactly this and answered *yes,
conditionally* — three conditions, each of which is a specification for this
change:

| Spike condition | Where it stands now |
|---|---|
| the catalogue needs a styled-span line node | **closed** by `ViewNode::Line` (ADR-28), plus `Fill` (ADR-31) for the residue a selection bar and a group rule reach across |
| selection must stay kernel state | **honoured**: the cursor is published per row, exactly as a task row's is; the plugin owns no cursor and receives no key |
| render must be event-driven, not a fixed poll | **still open**, and this port is where it bites. Recorded with its measurement rather than worked around |

Four panes have been ported (ADR-27/29/30/31) and one surface recorded as
structurally unportable (§10). This is the fifth port and the one the ADR names,
so the question it has to answer is not "can another list be drawn" but "does the
surface built for four easier panes hold for the hard one, unchanged".

The v1 behaviour being reproduced is `ui::project_list`'s
`render_session_section` — specifically the two line builders it composes rows
from:

- `group_header_line`: `── <repo label>` followed by a rule of `─` to the pane's
  edge, muted, never reflecting selection;
- `build_session_line`: a status glyph padded with a space either side, in that
  status's colour —
  **animated** through `ui::SPINNER_FRAMES` while the session is working — then
  the tree-nesting prefix (`└` per depth, or `↳` for a child whose parent
  renders in another group), the remote-host mark `⇅`, the worktree mark `⑂`,
  the session's name with the global search's matched characters emphasised, and
  the agent's activity text (or, when blocked, its notification) fitted to what
  is left of the column; with the row the cursor is on drawn in the theme's
  selection pair, bold, across the pane's whole width.

The spinner is the first real consumer of declared motion (ADR-V18), which
shipped with no bundled consumer at all. It is used here as declared motion —
ten frames at 8 fps, keyed, driven by the kernel's clock — for both the plugin
*and* the native pane, so the two cannot animate from different rules.

The native pane stays compiled in and stays the one on screen. Handover is Phase
6, and `tests/teardown_gate.rs` keeps this pane's row blocked while
`src/app/view.rs` still names `project_list`.

## What Changes

- **The session list becomes a published snapshot section.** `PaneContext` gains
  `session_list: SessionListSnapshot` — one entry per row in the order the pane
  renders them, each carrying the row's name, its status in drawable form, the
  repo-group label when it opens a group, its nesting depth, whether its parent
  renders elsewhere, whether it is remote, whether it is on a worktree, whether
  the cursor is on it, whether a running search dimmed it, the byte offsets that
  search matched, and the agent's activity and notification text. Bounded by
  `MAX_SESSION_ROWS`.
- **One new reader, under the capability that already exists for it.**
  `thurbox.sessionList()` is gated on `Capability::Sessions`, whose sentence is
  already "read the sessions thurbox is running" — plural. No new capability: a
  pane that draws the session list is asking the same question of the user as one
  that draws the active session's name, and splitting them would make the install
  prompt less honest, not more.
- **The native pane renders through the view tree.** `session_list_tree` and
  `session_item_node` are the presentation step and carry no geometry;
  `resolve_items` is the geometry step (which rows exist, what a group header
  says, how wide the activity text may be) and stays the kernel's. The native
  pane paints the same nodes a plugin pushes, through a new
  `ui::plugin_pane::line_spans`, so its rows keep going into the existing
  ratatui list and its scroll offsets, hitboxes and border chrome are untouched.
- **thurbox's own working spinner becomes declared motion.** The status glyph of
  a working row is a `motion` node of ten frames at 8 fps in both trees. The
  native pane resolves which frame is showing through a `FrameTable` filled from
  the app's existing spinner clock — the same data channel the kernel fills for
  a plugin — so `ui` still cannot reach a VM and the two panes cannot disagree
  about the frame rate or the frames.
- **A bundled `session-list` plugin**, shipped in the binary next to `hello`,
  `info-panel`, `tasks`, `file-viewer` and `code-review`, `default_visible =
  false`. It owns its spinner frames, its glyph choices, its colour roles and its
  matched-run segmentation. `tests/bundled_session_list.rs` asserts its tree
  equals the kernel's across content variants, that it paints the same frame, and
  that it declares every power it uses and no other.
- **The render-trigger finding is recorded and pinned.** The plugin worker
  re-renders on a fixed 1 s cycle, so a plugin's *copy* of the cursor can trail
  the native pane's by up to that interval. Selection is kernel state, so the
  cursor the user drives is unaffected — that is the spike's second condition
  doing its job — but the third condition is unmet, and the audit records the
  latency, its cause, and what closing it would cost.

## Capabilities

- `plugin-host/kernel-state` — ADDED: the session list as a published section,
  what it carries, what it deliberately does not, and the bound on it.
- `plugin-host/capabilities` — ADDED: the sessions capability covers the whole
  list and not only the active session, and why that is one question rather than
  two.
- `plugin-host/motion` — ADDED: a native pane resolves declared motion through
  the same frame table a plugin's pane does; thurbox's own working spinner is a
  declared motion rather than a second animation rule.
- `migration/phase-4` — ADDED: the pane ADR-V1 hinges on is reproduced by a
  bundled plugin; a spike's recorded conditions are re-checked at the port that
  depends on them; and a plugin's view of kernel state trails the kernel's by the
  render interval.

## Non-goals

Everything below belongs to `src/ui/project_list.rs` or its callers and is **not**
ported. Each line names why.

- **The pane's border chrome.** The `Sessions` block, the one-dot-per-session
  status strip on its top border, and the `▲ N` / `▼ N` clipped-row indicators on
  its borders. A plugin pane's frame is the host's (`focus_block` in
  `App::render_plugin_panes`), and nothing in the catalogue describes a border
  overlay — §9 recorded the same gap for the file viewer's search bar.
- **The empty state.** `No sessions yet` / `Press Ctrl+N to create one` are drawn
  **centred** in the pane. No node carries an alignment, so the plugin draws its
  empty state left-aligned and the two are compared only when rows exist. Recorded
  as a new open vocabulary row rather than approximated.
- **The pending-spawn placeholder row.** A row for a session that does not exist
  yet, inserted at an index the kernel computes from the group layout
  (`pending_spawn_slot`), whose phase label is dropped when the column is narrow.
  It is a second published shape and a second geometry rule; the rows that
  correspond to real sessions are what is being measured.
- **Scrolling exactly as the native pane scrolls.** The plugin declares which row
  its cursor is on and the kernel windows the list (ADR-30's mechanism); the
  native pane uses ratatui's own list offset. The two keep the cursor visible by
  different rules, so the comparison runs at a height where neither windows
  anything — asserted, not assumed.
- **Keys.** No `j`/`k`, no `Shift+J`/`Shift+K` reordering, no `Enter`. The
  plugin's pane is read-only; the cursor it draws is the kernel's.
- **Click hitboxes.** Row hitboxes are geometry the host resolves after the list
  widget has chosen its offset.
- **A `thurbox.format.*` helper table** (§7). This pane formats no byte counts and
  no durations either, so after five ports the case for one is still made by
  exactly one pane.
- **Deleting or unwiring the native pane.** Phase 6.

## Impact

- New code: `src/plugin/bundled/session-list/{plugin.toml,init.luau}`,
  `tests/bundled_session_list.rs`.
- Changed: `src/session/pane_context.rs`, `src/plugin/{capabilities.rs,
  discovery.rs,kernel_state.rs}`, `src/plugin/bundled/thurbox.d.luau`,
  `src/ui/{project_list.rs,plugin_pane.rs,highlight.rs}`,
  `src/app/{mod.rs,view.rs}`, `tests/bundled_info_panel.rs` (one fixture gains the
  new section), `docs/{PHASE4-PANE-READINESS.md,ARCHITECTURE.md}`, `CLAUDE.md`.
  `src/session/plugin_manifest.rs` changes by **one doc comment**: the `sessions`
  capability's sentence now says the grant covers every session, since the
  disclosure widens. Its vocabulary is untouched.
  `src/ui/highlight.rs`'s two span builders become `#[cfg(test)]`: the session
  list was their last caller, and they survive as the port's oracle.
- Unchanged on purpose: `src/session/view_tree.rs` (the catalogue needed nothing —
  the headline result), `tests/teardown_gate.rs` (this pane's row stays blocked),
  and the pane's border chrome, hitboxes and list offsets in `ui::project_list`.
- Feature gate: everything under `src/plugin/` stays behind
  `#[cfg(feature = "plugins")]`; the snapshot section and the tree builders are
  ungated kernel code, as `session::pane_context` and `session::view_tree` already
  are. `cargo tree --edges normal | grep -c mlua` stays 0.
- Architecture: no new edge. The section is pure data in `session`, the Lua
  conversion is `plugin → session`, the tree builders and the renderer are
  `ui → session`. The one place that must see both `ui::project_list` and
  `plugin::PluginHost` is an integration test, outside the library's module graph.
- Snapshots: none move. Every pinned frame shows an empty session list, whose
  drawing is untouched, and no pinned frame records style.
