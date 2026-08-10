# Render the file viewer's tree from a bundled Luau plugin

## Why

Phase 4 turns thurbox's native panes into bundled plugins, easiest first. The
info panel went first (ADR-27) and the tasks pane second (ADR-29). The tasks port
closed the list-row styling gap and left **two named geometry gaps**, recorded in
`docs/PHASE4-PANE-READINESS.md` §8, one of which it called a precondition of the
session-list port:

> more rows than the pane has lines → the kernel windows them around the
> selection; the plugin's copy draws from the first row, so a selection below the
> fold is invisible. *Cheapest closure: a list node carrying a selected index,
> windowed by the kernel from the height it has — the `gauge` shape, applied to
> height.*

The file viewer is the pane that makes closing it unavoidable. Its whole
interaction is scrolling a tree taller than its column, so a copy that cannot
scroll to its cursor is not a reproduction of it — where the tasks pane could
record the gap and move on, this one cannot.

The v1 behaviour being reproduced is the tree half of `src/ui/file_viewer.rs`:
the expandable directory tree of every worktree and additional directory of the
active session, drawn one row per visible node as
`<indent><marker><name>` — `▸`/`▾` for a collapsed/expanded directory and two
spaces for a file, or the nerd-font `` / `` / `` when nerd glyphs are
enabled — with a matched directory in accent and bold, a matched file in the
theme's primary text, every row a running search excluded in muted, the row the
cursor is on in the theme's selection colours (bold for its name), and a muted
`No folders` line when the session has no directories. The window scrolls to keep
the cursor visible.

Today `src/app/view.rs` hands `FileViewerState` to a Rust function on the UI
thread, which flattens it, windows it, and builds ratatui spans. After this
change a Luau plugin in its own VM, on the plugin render worker, reads the same
rows through one capability-gated binding and returns the same view tree — and
the kernel, not the plugin, still resolves the window.

The native pane stays compiled in and stays the one on screen. Handover is
Phase 6 and `tests/teardown_gate.rs` keeps this pane's row blocked while
`src/app/view.rs` still names `file_viewer`.

## What Changes

- **A list may carry the row its cursor is on**, and the kernel windows it.
  `ViewNode::List` gains `selected: Option<usize>`; when present the renderer
  chooses the visible slice with the same `visible_window` the native panes use,
  so a plugin's list scrolls to its selection without ever learning a height.
  This closes §8's second geometry gap — for this pane, for the tasks pane, and
  for the session list that comes after it.
- **A run may declare that it belongs to the selected row.** `TextStyle` gains
  `selected: bool`, which the renderer resolves to the theme's selection
  foreground on its selection background. It is not an emphasis: it *replaces*
  the run's colour rather than layering an attribute over it, because a selected
  row in this pane is a background and the tree had no way to name one. The
  plugin still names a **role**, not a colour — the theme owns both halves of the
  pair, exactly as it owns what `accent` resolves to.
- **A `files` capability and one reader.** `thurbox.files()` returns the rows of
  the tree the file viewer currently has open. Scoped deliberately narrowly: a
  row is a **basename**, its depth, whether it is a directory, whether it is
  expanded, and whether a running search matched it — plus which row the cursor
  is on and whether nerd glyphs are enabled. It grants **no filesystem access at
  all**: no directory listing, no file contents, no path. See `design.md` §1 for
  why the wider "list a directory, read a file's lines" shape was rejected rather
  than merely deferred.
- **A `files` section on the published snapshot.** `PaneContext` gains
  `files: FilesSnapshot`, bounded by `MAX_FILE_ROWS` so a tree with a large
  directory expanded in it cannot exceed the view tree's node budget, and empty
  when the `file_viewer` feature is off — mirroring the task and automation
  sections.
- **The native pane draws its view tree.** `ui::file_viewer::file_tree` becomes
  the pane's rendering IR the way `tasks_tree` is the tasks pane's, painted by
  the shared `ui::plugin_pane::render_tree`. It is **geometry-free**: it takes
  every row and the selected index, and the renderer resolves the window. The
  pane keeps computing that window for its click hitboxes and its scrollbar, from
  the same function, and a test asserts the two agree.
- **A bundled `file-viewer` plugin**, shipped in the binary next to `hello`,
  `info-panel` and `tasks`, `default_visible = false`. It owns the marker glyphs
  (both glyph sets), the indentation, the colour roles, and the empty-state line;
  `tests/bundled_file_viewer.rs` asserts its tree **equals** the native pane's
  across content variants, and — because the window is now the kernel's — that
  the two **paint the same frame** at a size where the native pane scrolls.

## Capabilities

- `plugin-host/view-tree` — ADDED: a list may carry its selected row and the
  kernel windows it to the height it has; a run may declare it belongs to the
  selected row, resolved to the theme's selection pair.
- `plugin-host/kernel-state` — ADDED: the open file tree as a published section,
  what it carries, the bound on it, and the filesystem powers it explicitly does
  not confer.
- `plugin-host/capabilities` — ADDED: reading the open file tree is its own
  declared capability, and it is not a filesystem capability.
- `migration/phase-4` — ADDED: the third ported pane; a pane's scroll window is
  resolved by the kernel from a declared selection rather than by reporting a
  rect; and a pane sub-mode the host surface cannot express is declared out of
  scope in the proposal with the missing nodes named, never quietly omitted.

## Non-goals

- **The search sub-mode's bar.** The file viewer's `/` search draws a three-row
  bordered ` Search (2/5) ` block below the tree, with a `/ ` prefix, the query
  text scrolled to its end, and a block cursor. The host surface cannot express
  any of the three things that needs: a bordered container node, a cursor
  appearance, and a fixed-height region anchored to the bottom of a pane — and
  the match counter needs the query text, which the `files` capability
  deliberately does not publish. It is therefore **out of scope and stated as
  such**, not silently omitted; `docs/PHASE4-PANE-READINESS.md` §9 records the
  three missing pieces. The search's *effect on the tree* — which rows are
  matched, and how a matched directory is emphasised — **is** ported, and is part
  of the equality test.
- **The scrollbar.** It is chrome outside the tree: `scrollbar::reserve_track`
  narrows the rect before the tree is painted, like the pane border. A plugin
  pane has no scrollbar; the divergence is pinned by a test with its closure
  named.
- **A filesystem capability.** Nothing here lets a plugin read a directory or a
  file. `Capability::Fs` remains undeclared, which is what
  `tests/teardown_gate.rs` reserves for the v1 "place a file in an agent's config
  dir" power — that row stays blocked.
- **Keys in the plugin's pane.** `j`/`k`/`Enter`/`/` act on thurbox's own tree;
  the plugin's copy is read-only, and it draws no key hints for actions it cannot
  perform.
- **Deleting or unwiring the native pane.** Phase 6.
- **A `thurbox.format.*` helper table** (§7). This pane formats nothing either,
  so the case for one is still made by zero panes.
- **Porting a fourth pane.** Exactly one, completely.

## Impact

- New code: `src/plugin/bundled/file-viewer/{plugin.toml,init.luau}`,
  `tests/bundled_file_viewer.rs`.
- Changed: `src/session/{view_tree.rs,pane_context.rs,plugin_manifest.rs}`,
  `src/plugin/{capabilities.rs,discovery.rs,kernel_state.rs,view.rs,pane.rs,lifecycle.rs}`,
  `src/plugin/bundled/thurbox.d.luau`, `src/ui/{file_viewer.rs,plugin_pane.rs,info_panel.rs,tasks_panel.rs}`,
  `src/app/{mod.rs,acceptance.rs}`, `docs/{PHASE4-PANE-READINESS.md,ARCHITECTURE.md}`,
  `CLAUDE.md`.
- `ViewNode::List` becomes a struct variant, so every construction site moves to
  the `ViewNode::list` constructor. Mechanical, and it is what stops a second
  list kind existing for the selectable case.
- Feature gate: everything under `src/plugin/` stays behind
  `#[cfg(feature = "plugins")]`; the snapshot section, the list's selected index
  and the pane's tree builder are ungated kernel code, as `session::view_tree`
  and `session::pane_context` already are. `cargo tree --edges normal |
  grep -c mlua` stays 0.
- Architecture: no new edge. The section is pure data in `session`; the Lua
  conversion is `plugin → session`; the tree builder and the windowing renderer
  are `ui → session`. `tests/architecture_rules.rs` is unchanged, and the one
  place that must see both `ui::file_viewer` and `plugin::PluginHost` is an
  integration test, outside the library's module graph.
- Snapshots: none move. No pinned frame contains the file viewer, and the pane's
  own rendering is unchanged — which the retained cell-level differential against
  the pre-port span renderer is what checks.
