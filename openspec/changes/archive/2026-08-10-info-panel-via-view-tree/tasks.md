# Tasks

## 1. Pin the pre-port frame

- [x] Add `info_panel_full_frame` to `src/ui/info_panel.rs`'s test module: a
  fixture with every optional row populated (parent, host, degraded hooks, a
  wrapping activity, a signal, repos plus two extra dirs, both git rows, session
  CPU/RAM, the full agent section, two usage windows, system CPU/RAM, disk, two
  automations) rendered at 40×46 into a `TestBackend`. Generate the snapshot
  from the **unmodified v1 renderer**, so it is a recording and not a
  restatement.
  Verify: `INSTA_UPDATE=always cargo test --lib ui::info_panel::tests::info_panel_full_frame`
  then `cargo nextest run -E 'test(info_panel_full_frame)'`

## 1a. Ungate the view tree

- [x] `src/session/mod.rs`, `src/ui/mod.rs`: drop `#[cfg(feature = "plugins")]`
  from `session::motion`, `session::view_tree` and `ui::plugin_pane`. Without
  this the criterion is unsatisfiable — see `design.md` §1a.
  Verify: `cargo check --all` then
  `cargo tree --edges normal | sort -u` unchanged, and
  `cargo tree --edges normal | grep -c mlua` → 0

## 2. Widen the catalogue

- [x] `src/session/view_tree.rs`: add the eleven `StyleToken` variants
  (`secondary`, `role`, `branch`, `added`, `border`, and one per session status),
  extending `as_str`/`all`/`parse`.
  Verify: `cargo nextest run -E 'test(style_tokens_round_trip)'`
- [x] `src/session/view_tree.rs`: add `Percent` (Eq/Hash by `to_bits`),
  `ViewNode::Gauge { label, percent, suffix }` and
  `ViewNode::Paragraph(Vec<ViewNode>)`; extend `children`, `kind_name`,
  `first_non_inlineable` (both are non-inlineable) and the constructors.
  Verify: `cargo nextest run -E 'test(view_tree)'`

## 3. Render the new nodes

- [x] `Cargo.toml`: enable ratatui's `unstable-rendered-line-info` so
  `Paragraph::line_count` is callable — the only way to ask ratatui how many rows
  its own wrapper will produce. Adds no crate to the graph.
- [x] `src/ui/plugin_pane.rs`: extend `token_color` with the eleven new arms;
  make `height_of` width-aware; render `Gauge` (the v1 geometry moved verbatim,
  header rows + 1 — its header wraps, see `design.md` §3a) and `Paragraph`
  (ratatui `Wrap { trim: false }`, height from `Paragraph::line_count`).
  Verify: `cargo nextest run -E 'test(ui::plugin_pane)'`

## 4. Port the pane

- [x] `src/ui/info_panel.rs`: replace the `Vec<Line>` builders with `ViewNode`
  builders and paint through `ui::plugin_pane::render_tree` inside the existing
  `Block`. Keep the v1 builders as a `#[cfg(test)]` oracle (`legacy_lines`).
  Verify: `cargo nextest run -E 'test(ui::info_panel)'`
- [x] `src/ui/info_panel.rs`: add the differential test — render the tree and
  the oracle into two buffers over a matrix of widths (18…60) and content
  variants (empty session, local session, full fixture, long values) and assert
  every cell agrees in symbol, foreground and modifiers, excluding space cells
  (divergence 1 in `design.md`).
  Verify: `cargo nextest run -E 'test(matches_the_legacy)'`
- [x] `src/ui/info_panel.rs`: pin each enumerated divergence with its own test.
  Verify: `cargo nextest run -E 'test(ui::info_panel)'`
- [x] Confirm the pinned frame did not move.
  Verify: `cargo nextest run --all` (no snapshot pending in `src/ui/snapshots/`)

## 5. Give plugins the same surface

- [x] `src/plugin/view.rs`: convert the `gauge` and `paragraph` kinds, refusing
  a non-finite percentage and a non-inlineable paragraph child.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::view)'`
- [x] `src/plugin/capabilities.rs`: add the two constructors to the frozen `ui`
  table and update the constructor-name test.
  Verify: `cargo nextest run --features plugins -E 'test(plugin::capabilities)'`
- [x] `src/plugin/bundled/thurbox.d.luau`: declare `ui.gauge` / `ui.paragraph`
  and the new tokens.
  Verify: `./scripts/dev/lint-luau.sh`

## 6. Docs

- [x] `docs/PHASE4-PANE-READINESS.md`: mark §3 and §4 closed with the commit
  that closed them, and state that §2 kept the info panel from being a plugin.
- [x] `docs/ARCHITECTURE.md`: new ADR recording the port and its three
  widenings.
- [x] `CLAUDE.md`: note that the info panel renders through the view tree and
  that the catalogue carries gauge/paragraph.
  Verify: `rumdl check .`

## 7. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `./scripts/dev/lint-luau.sh` ; `rumdl check .`
