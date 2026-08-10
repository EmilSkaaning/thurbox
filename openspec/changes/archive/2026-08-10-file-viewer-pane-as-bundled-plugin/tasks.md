# Tasks

## 1. A list that carries its cursor

- [x] `src/session/view_tree.rs`: `ViewNode::List` becomes
  `List { children, selected: Option<usize> }`, with `ViewNode::list` and
  `ViewNode::selectable_list` constructors; `children()`, `kind_name`,
  `first_non_inlineable` follow. Move every construction site to the constructor.
  Verify: `cargo nextest run -E 'test(session::view_tree)'`
- [x] `src/ui/plugin_pane.rs`: a selected list windows its children through
  `file_viewer::visible_window`; tests for a list that fits, a list that scrolls
  to a selection below the fold, and one with no selection.
  Verify: `cargo nextest run -E 'test(ui::plugin_pane)'`
- [x] `src/plugin/view.rs`: convert `selected` on a `list` node — one-based,
  refused when zero, negative, non-integer, or past the last child.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::view)'`
- [x] `src/plugin/capabilities.rs`: `ui.list(children, selected?)` split out of the
  shared container loop.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`

## 2. A run that belongs to the selected row

- [x] `src/session/view_tree.rs`: `TextStyle::selected`, documented as replacing
  the token's colour rather than layering.
  Verify: `cargo nextest run -E 'test(session::view_tree)'`
- [x] `src/ui/plugin_pane.rs`: `text_style` resolves it to
  `selection_fg`/`selection_bg`; a test asserts it overrides the token, composes
  with bold, and does not bleed onto the next run.
  Verify: `cargo nextest run -E 'test(ui::plugin_pane)'`
- [x] `src/plugin/view.rs` + `src/plugin/capabilities.rs`: the sixth positional
  flag on `ui.text`, with a test for it and for its absence.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::)'`

## 3. The published file section

- [x] `src/session/pane_context.rs`: `FileNodeSnapshot` / `FilesSnapshot`,
  `PaneContext::files`, `MAX_FILE_ROWS` with its rationale. Tests: structural
  equality (the change gate needs it) and that a truncated section drops its
  cursor rather than publishing an index outside the rows.
  Verify: `cargo nextest run -E 'test(session::pane_context)'`
- [x] `src/session/plugin_manifest.rs`: `Capability::Files`, wire name `files`, in
  `as_str`, `all`, `reads_kernel_state`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin_manifest)'`
- [x] `src/plugin/kernel_state.rs`: `files_table` — an always-present table of
  `nodes` (1-based), an optional 1-based `selected`, and `nerdFont`.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::kernel_state)'`
- [x] `src/plugin/capabilities.rs`: insert `files` under `Capability::Files`;
  extend the per-capability gating tests so one grant still implies no other.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`

## 4. The native pane renders its tree

- [x] `src/ui/file_viewer.rs`: `FileRow`, `FileViewerState::rows` /
  `selected_index`, and the geometry-free `file_tree(rows, selected, nerd_font)`;
  `render_file_viewer` paints it through `plugin_pane::render_tree` while keeping
  its hitboxes and scrollbar. Retain the span-based row builder as a
  `#[cfg(test)]` oracle and assert the tree paints cell-for-cell identically
  across every row appearance, including the windowed case and both glyph sets.
  Verify: `cargo nextest run -E 'test(ui::file_viewer)'`
- [x] `src/ui/file_viewer.rs`: a test that the hitboxes cover exactly the rows the
  renderer drew, since the window is now resolved twice from one function.
  Verify: `cargo nextest run -E 'test(ui::file_viewer)'`

## 5. Publish it from `app`

- [x] `src/app/mod.rs`: `build_pane_context` fills the file section — bounded,
  empty with `features.file_viewer` off, cursor dropped past the bound.
  Verify: `cargo nextest run -E 'test(pane_context)'`
- [x] `src/app/acceptance.rs`: assert the published section reflects the open tree
  and its cursor, is empty with the feature off, and that publishing stays
  change-gated (no new `pane_context_publishes` on an idle tick).
  Verify: `cargo nextest run -E 'test(pane_context)'`

## 6. The bundled plugin

- [x] `src/plugin/bundled/file-viewer/plugin.toml`: `capabilities = ["render",
  "files"]`, one pane, `default_visible = false`.
- [x] `src/plugin/bundled/file-viewer/init.luau`: both marker glyph sets, the
  indentation, the selected / matched / unmatched colour roles, the directory
  emphasis, the empty-state line, and the selected index on its list.
- [x] `src/plugin/discovery.rs`: add it to `BUNDLED`.
  Verify: `cargo nextest run --features plugins -E 'test(bundled)'`
- [x] `src/plugin/bundled/thurbox.d.luau`: the file types, the reader, the list's
  `selected`, and the `selected` flag on `ui.text`.
  Verify: `./scripts/dev/lint-luau.sh`

## 7. Prove it renders the same pane

- [x] `tests/bundled_file_viewer.rs` (new, `#![cfg(feature = "plugins")]`): tree
  equality against `file_tree` over content variants; **frame** equality at a size
  where the pane scrolls (the property the previous port could not claim); the
  plugin declares exactly `render` + `files`; the two remaining divergences
  (search bar, scrollbar) pinned with their closures; and that the plugin holds no
  binding that touches the filesystem.
  Verify: `cargo nextest run --features plugins --test bundled_file_viewer`
- [x] `tests/teardown_gate.rs`: unchanged, and still records the file-viewer row
  blocked because `src/app/view.rs` names `file_viewer`, and the `Fs` row blocked
  because no filesystem capability was added.
  Verify: `cargo nextest run --test teardown_gate`

## 8. Docs

- [x] `docs/PHASE4-PANE-READINESS.md` §9: the third port — what sufficed, the two
  widenings, §8's second geometry row closed, the search bar's three missing
  pieces, and the section's lazy-tree limitation.
- [x] `docs/ARCHITECTURE.md`: an ADR for the selected list, the selection style
  role, and the `files` capability's bound (with the rejected filesystem shape).
- [x] `CLAUDE.md`: the `files` capability, the bundled pane, the selected list.
  Verify: `rumdl check .`

## 9. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (≥ 2167, 0 failed)
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
  (≥ 2514, 0 failed)
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `./scripts/dev/lint-luau.sh` ; `rumdl check .`
