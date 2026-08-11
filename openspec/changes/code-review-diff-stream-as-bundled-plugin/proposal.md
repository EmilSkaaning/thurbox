# Render the code review's unified diff stream from a bundled Luau plugin

## Why

Phase 4 turns thurbox's native panes into bundled plugins. Three have landed —
the info panel (ADR-27), the tasks pane (ADR-29) and the file viewer (ADR-30) —
and the fourth surface on the list, global search, was recorded as **not a pane
at all** (`docs/PHASE4-PANE-READINESS.md` §10). Code review is the next surface,
and it is the largest: `src/ui/code_review.rs` draws a unified *and* a paired
side-by-side diff, syntax-highlighted bodies, classified comments, reviewed
marks, a find sub-mode, a target picker, horizontal scroll, a wrap toggle, a
footer of buttons and a floating compose box.

Porting all of that would answer nothing well. What it *can* answer, and what no
port so far has, is the question the view tree has not been asked: **can a plugin
style the inside of a line, thousands of times, within the host's bounds?** Every
pane ported so far draws one row per record with two or three runs on it. A diff
line is a gutter plus one run per syntax token plus a background that has to reach
the pane's right edge, and a real diff is thousands of those.

So this change ports **the unified diff stream's lines** — completely — and
declares the rest of the pane out of scope, itemised, in Non-goals. The v1
behaviour being reproduced is `ui::code_review`'s `unified_diff_line`: for each
line of each hunk, a `{old:>w} {new:>w} {sign} ` line-number gutter drawn in the
muted role, then the line's body tokenised by `ui::syntax` and drawn one run per
token, then padding to the pane's width — with the whole row tinted by
`diff_added_bg` on an insertion and `diff_removed_bg` on a deletion, and the row
the cursor is on drawn in the theme's selection pair instead, which is what
`row_bg_fn` encodes.

Unlike the three earlier ports, the native renderer is **not** refactored to draw
the tree it is compared against, and that is a deliberate departure argued in
`design.md` §2: this pane's painter is width-dependent in ways no view tree can
express (a horizontal scroll window, a wrap that reflows one logical row onto
several, a body sliced by character count against a width). The oracle is
therefore **frame equality against the untouched native renderer** — the tree
builder is painted into a buffer, `unified_diff_line` is painted into another, and
the two buffers must be identical cell for cell. That bridge is narrower in scope
than the earlier ports' and stronger in kind: it compares against what the pane
actually paints today rather than against a refactor of it.

The native pane stays compiled in and stays the one on screen. Handover is
Phase 6, and `tests/teardown_gate.rs` keeps this pane's row blocked while
`src/app/view.rs` still names `code_review`.

## What Changes

- **A run may declare that its row is an insertion or a deletion.** `TextStyle`
  gains `tint: Option<DiffTint>` with exactly two members, resolved by the host to
  the theme's `diff_added_bg` / `diff_removed_bg`. It is a **role**, like
  `TextStyle::selected` and unlike the three emphases: the plugin names which kind
  of diff row the run is on and the theme owns the colour. Selection wins over a
  tint, which is the rule `ui::code_review::row_bg_fn` already encodes — without
  that precedence a selected insertion would draw in two backgrounds at once.
- **A run may be a fill.** `ViewNode::Fill { glyph, style }` is an inline run that
  expands to whatever width is left on its line after every other run has taken
  its own. It exists because a background that stops at the end of the text is not
  the native pane's row tint: the tint reaches the pane's right edge, and a plugin
  cannot pad to a width it is never told. This is the `gauge` trade (ADR-26)
  applied to a line's residue, and it is the first half of the flush-right run
  `docs/PHASE4-PANE-READINESS.md` §8 has had open since the tasks port.
- **One more style token.** `StyleToken::AccentBright` resolves to the palette's
  `accent_bright`, which is the colour `ui::syntax` gives a capitalised type name
  and the only one of the six colours the highlighter uses that no token could
  name. The other five are already reachable (`muted`, `branch`,
  `status_working`, `accent`, and the token-less primary foreground).
- **A text style may be given as a table.** `ui.text(content, style)` continues to
  accept a token name and positional flags, and now also accepts
  `{ token = …, bold = …, dim = …, underline = …, selected = …, tint = … }`. The
  positional form was documented as full at six arguments, and `tint` is the
  seventh — so the table is the form that note asked for, added without changing
  what any existing plugin passes.
- **A `review` capability and one reader.** `thurbox.review()` returns the diff
  stream's lines: per line a repository-relative path, the old and new line
  numbers where each exists, whether the line is an addition, a deletion or
  context, and its text — plus which row the cursor is on and the width the
  gutter's number columns are drawn at. It reads no repository and runs no `git`:
  the section is built from the diff the review already has open, exactly as the
  `files` section is built from the tree the file viewer already has open.
- **A `review` section on the published snapshot.** `PaneContext` gains
  `review: ReviewSnapshot`, bounded by `MAX_REVIEW_ROWS` and empty when the
  `code_review` feature is off — mirroring the task, automation and file sections.
- **The sandbox loads the UTF-8 library.** A pane that styles the inside of a line
  has to agree with the host about where a character ends, and the VM loaded no way
  to walk a string by character — `string.byte` counts bytes, so a plugin lexing a
  line containing one multi-byte character drifts for the rest of it. `utf8` is
  pure computation (no file, no process, no clock), so it is admissible under the
  restricted-environment rule for the same reason `math` is. Discovered by the
  port, not predicted by the audit.
- **A bundled `code-review` plugin**, shipped in the binary next to `hello`,
  `info-panel`, `tasks` and `file-viewer`, `default_visible = false`. It owns its
  gutter format, its colour roles, **and its own syntax highlighter** — a Luau
  lexer, not a published token stream, for the reason `design.md` §1 gives.
  `tests/bundled_code_review.rs` asserts its tree equals the kernel's tree builder
  across content variants, and the tree builder is in turn pinned to the native
  renderer by frame equality in `ui::code_review`'s own tests.
- **The view tree's node budget is measured against a real pane, and it does not
  fit.** `MAX_NODES` is 4096 for a whole tree while a diff line costs one node per
  syntax token, so the stream is publishable only as a bounded window and even a
  bounded window can be refused by a pathological line. The measurement, the cap
  it forces, and the two ways out are recorded in
  `docs/PHASE4-PANE-READINESS.md` §11 and pinned by a test.

## Capabilities

- `plugin-host/view-tree` — ADDED: a run may declare its row is an insertion or a
  deletion, resolved to the theme's diff backgrounds with selection winning; a run
  may be a fill that consumes a line's remaining width; the palette's bright
  accent is addressable; a text style may be given as a table.
- `plugin-host/kernel-state` — ADDED: the open review's diff lines as a published
  section, what it carries, the bound on it, and the repository powers it does not
  confer.
- `plugin-host/capabilities` — ADDED: reading the open review's diff is its own
  declared capability, and it is not a git capability.
- `plugin-host/runtime` — ADDED: a plugin may walk a string by character, and the
  library that lets it grants no ambient access.
- `migration/phase-4` — ADDED: a pane may be ported in part when its whole is not
  expressible, provided the boundary is itemised; a reproduction whose native pane
  is not refactored is validated by frame equality against the untouched renderer;
  and the node budget is a whole-tree bound that a per-row pane cannot respect
  locally.

## Non-goals

Everything below is part of `src/ui/code_review.rs` and is **not** ported. Each
line names why, so the remaining surface is a list rather than a gap.

- **The side-by-side layout.** `paired_diff_line` splits the pane into two half
  cells and aligns a deletion against its addition. Both halves need a resolved
  width (`paired_body_width` divides it in two), which no node carries.
- **The wrap toggle.** `unified_diff_line_wrapped` turns one logical row into as
  many rows as its body needs, chunked by the *available* width. A view tree's
  `Paragraph` wraps, but the native pane's chunk boundaries are its own arithmetic
  over a width the plugin is never told, so the two cannot agree.
- **Horizontal scroll.** The body is windowed to `[h_scroll, h_scroll + avail)`.
  Both bounds are geometry.
- **File headers and hunk headers.** Expressible in shape — the fill node this
  change adds is what a file header's trailing rule needs — but the header's
  add/remove counts are drawn in `diff_added`/`diff_removed`, which are separate
  palette fields from the `added` token's `tool_allowed`, and the hunk header is
  `truncate`d with an ellipsis, which is §8's still-open clipping row. Both are
  out of scope here so that this port adds one style role and one node rather than
  four.
- **Comments, their classification badges, and the review summary rows.** They are
  a second published shape (bodies, classifications, anchors) and a second
  interaction; the diff stream is the one being measured.
- **Reviewed marks and folding.** A `✓` on a file or hunk header, and a folded
  file collapsing its rows — both belong to the headers, above.
- **The find sub-mode.** Its bar needs the three host features
  `docs/PHASE4-PANE-READINESS.md` §9 already named for the file viewer's search
  bar (a frame node, a cursor appearance, a bottom-anchored region), and its
  in-row match highlight replaces the syntax colouring for a matched line, which
  is a third styling mode.
- **The target picker, the footer buttons and the compose box.** Chrome and
  sub-modes, each owning keys the plugin's pane does not receive.
- **The scrollbar.** Chrome outside the rows, exactly as for the file viewer.
- **The central-pane seat.** The native review owns the middle of the screen and a
  column in the right slot; `PaneSlot` seats a plugin pane only on the right. The
  plugin's pane is therefore a narrow copy in the right column, which is what
  every earlier port also is.
- **Keys.** The plugin's pane is read-only: nothing in it selects a row, marks a
  file reviewed, or opens a comment.
- **Refactoring the native renderer to draw the tree.** Argued in `design.md` §2.
- **Deleting or unwiring the native pane.** Phase 6.
- **A `thurbox.format.*` helper table** (§7). This pane formats no byte counts and
  no durations either, so the case for one is still made by exactly one pane after
  four ports.

## Impact

- New code: `src/plugin/bundled/code-review/{plugin.toml,init.luau}`,
  `tests/bundled_code_review.rs`.
- Changed: `src/session/{view_tree.rs,pane_context.rs,plugin_manifest.rs}`,
  `src/plugin/{capabilities.rs,discovery.rs,kernel_state.rs,runtime.rs,view.rs}`,
  `src/plugin/bundled/thurbox.d.luau`, `src/ui/{code_review.rs,plugin_pane.rs}`,
  `src/app/mod.rs`, `docs/{PHASE4-PANE-READINESS.md,ARCHITECTURE.md,CONFIG.md}`,
  `CLAUDE.md`.
- Unchanged on purpose: `src/app/view.rs` (the native pane is still what it
  draws), the native paint path in `ui::code_review` (so no pinned frame can
  move), and `tests/teardown_gate.rs` (this pane's row stays blocked).
- Feature gate: everything under `src/plugin/` stays behind
  `#[cfg(feature = "plugins")]`; the new node, the new style role, the snapshot
  section and the tree builder are ungated kernel code, as `session::view_tree`
  and `session::pane_context` already are. `cargo tree --edges normal |
  grep -c mlua` stays 0.
- Architecture: no new edge. The section is pure data in `session`, the Lua
  conversion is `plugin → session`, the tree builder and the renderer are
  `ui → session`. `tests/architecture_rules.rs` is unchanged, and the one place
  that must see both `ui::code_review` and `plugin::PluginHost` is an integration
  test, outside the library's module graph.
- Snapshots: none move. No pinned frame contains the code review view, and the
  native pane's rendering is not touched.
