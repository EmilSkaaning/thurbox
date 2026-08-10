# Tasks — the inline line node

## 1. The node and its rule (pure data)

- [x] 1.1 `ViewNode::Line(Vec<ViewNode>)` in `session/view_tree.rs`; its runs are
      its `children()` so the existing node/depth accounting covers them.
- [x] 1.2 `ViewNode::is_inlineable()` — recursive: text yes, line all-children,
      motion all-frames, everything else no.
- [x] 1.3 Tests: runs count toward `node_count`/`depth`; the predicate answers
      correctly for every kind including a motion smuggling a column.

**Verify:** `cargo nextest run --features plugins -E 'test(view_tree)'`

## 2. Conversion

- [x] 2.1 `plugin/view.rs`: convert `kind = "line"`, reusing `convert_children`
      so the structural path (motion identity) threads through unchanged.
- [x] 2.2 `ViewError::NotInlineable { kind }` and its message.
- [x] 2.3 Tests: a line of text converts; a column/divider/spacer child is
      refused by name; a motion with inlineable frames converts; a motion with a
      column frame is refused; a nested line converts.

**Verify:** `cargo nextest run --features plugins -E 'test(plugin::view)'`

## 3. Layout and paint

- [x] 3.1 `ui/plugin_pane.rs`: `inline_width` over `unicode-width` — text is its
      display width, a nested line the sum, a motion the max over frames.
- [x] 3.2 `height_of` returns 1 for a line.
- [x] 3.3 Flatten a line to spans in order, resolving a motion's frame from the
      `FrameTable` and padding it to the reserved width; render as one
      `Paragraph` line so clipping is the terminal's.
- [x] 3.4 Tests: two styles on one row; unequal runs not equalised; overflow
      clipped; a motion pads so the next run's column is frame-independent;
      wide (CJK) runs measured by display width, not char count.

**Verify:** `cargo nextest run --features plugins -E 'test(plugin_pane)'`

## 4. The authored surface

- [x] 4.1 `ui.line(children)` in `plugin/capabilities.rs`, in the frozen table.
- [x] 4.2 `bundled/thurbox.d.luau`: declare `line` on `Ui`.
- [x] 4.3 `bundled/hello/init.luau`: draw one `label: value` line, since the
      example is what an author copies.
- [x] 4.4 Tests: the constructor is present with no capabilities granted; the
      table is still frozen.

**Verify:** `./scripts/dev/lint-luau.sh` and
`cargo nextest run --features plugins -E 'test(capabilities)'`

## 5. Docs

- [x] 5.1 `CLAUDE.md`: name `line` among the constructors in the plugin
      paragraph, with the one-clause reason it is not `row`.

**Verify:** `rumdl check .`

## 6. Full verification

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --all-targets --features plugins -- -D warnings` and the
      default-feature run
- [x] 6.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 6.4 `cargo nextest run --all` and `--all --features plugins`
- [x] 6.5 `cargo tree --edges normal | grep -c mlua` is 0
