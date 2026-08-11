# Tasks

## 1. The view tree gains a tint, a fill and a token

- [x] 1.1 `src/session/view_tree.rs`: add `DiffTint` (`Added`/`Removed`, wire
      `added`/`removed`), `TextStyle::tint`, `ViewNode::Fill { glyph, style }`, and
      `StyleToken::AccentBright`. Extend `ViewNode::children`, `is_inlineable`,
      `kind_name`, `depth`/`count` walks and the unit tests.
- [x] 1.2 `src/ui/plugin_pane.rs`: `token_color` resolves the new token;
      `text_style` paints a tint as a background with selection winning; the line
      paint path expands a `Fill` to the width left over; `inline_width` reports a
      fill as zero.
- [x] 1.3 `src/plugin/view.rs`: convert the `tint` field and the `fill` node kind,
      refusing an unknown tint and a fill outside an inline container. Unit tests
      per refusal.
- [x] 1.4 `src/plugin/capabilities.rs`: `ui.fill(glyph, style)`, and `ui.text`'s
      second argument accepts a style **table** as well as a token name.
- [x] Verify: `cargo nextest run --features plugins -E 'test(view_tree) + test(plugin::view) + test(plugin_pane)'`

## 2. The review section and its capability

- [x] 2.1 `src/session/plugin_manifest.rs`: `Capability::Review` (wire `review`),
      in `all()` and in the state-reading set.
- [x] 2.2 `src/session/pane_context.rs`: `ReviewSnapshot`, `ReviewLineSnapshot`,
      `MAX_REVIEW_ROWS`, and the `review` field on `PaneContext`.
- [x] 2.3 `src/plugin/kernel_state.rs`: `review_table`, one entry per line, with
      unit tests for the empty section and a capped one.
- [x] 2.4 `src/plugin/capabilities.rs`: `thurbox.review()` gated on the capability,
      plus the test that an ungranted plugin finds no binding.
- [x] 2.5 `src/app/mod.rs`: `build_review_snapshot`, called from
      `build_pane_context`; empty with the `code_review` feature off; cursor
      dropped when it falls past the cap.
- [x] Verify: `cargo nextest run --features plugins -E 'test(pane_context) + test(kernel_state) + test(capabilit)'`

## 3. The kernel's tree builder, pinned to the native renderer

- [x] 3.1 `src/ui/code_review.rs`: `pub fn diff_row_tree(path, line, num_w,
      selected) -> ViewNode` and `pub fn diff_stream_tree(rows, cursor, num_w)`,
      both geometry-free, using `ui::syntax` for the body.
- [x] 3.2 In the same module's tests: paint `diff_row_tree` and the untouched
      `unified_diff_line` at the same width and assert the buffers are identical,
      for an addition, a deletion, context, the cursor's row, an empty body, and a
      line of each token kind.
- [x] Verify: `cargo nextest run -E 'test(code_review)'`

## 3b. The sandbox learns to walk characters

- [x] 3b.1 `src/plugin/runtime.rs`: add `StdLib::UTF8` to the loaded set, with the
      reason (pure computation; a pane that styles inside a line must count
      characters the way the host does).
- [x] Verify: `cargo nextest run --features plugins -E 'binary(bundled_code_review)'`
      — the multi-byte case is what fails without it.

## 4. The bundled plugin

- [x] 4.1 `src/plugin/bundled/code-review/plugin.toml` — `render` + `review`,
      one pane, `default_visible = false`.
- [x] 4.2 `src/plugin/bundled/code-review/init.luau` — the gutter, the tint, the
      fill, and the Luau port of `ui::syntax` (`lang_for`, the keyword union, the
      four scanners, `word_color`).
- [x] 4.3 `src/plugin/discovery.rs`: add it to `BUNDLED`.
- [x] 4.4 `src/plugin/bundled/thurbox.d.luau`: `ui.fill`, the style table, the
      `tint` field, `thurbox.review`, `ReviewLine`.
- [x] Verify: `./scripts/dev/lint-luau.sh`

## 5. The equality test

- [x] 5.1 `tests/bundled_code_review.rs`: tree equality against
      `diff_stream_tree` across content variants (addition, deletion, context, the
      cursor, every token kind, a multi-byte body, an empty stream, a cursor past
      the cap); the capability-set test; the default-visible test; and the node
      budget measured, with the row cost and the refusal both asserted.
- [x] Verify: `cargo nextest run --features plugins -E 'test(bundled_code_review)'`

## 6. Documentation

- [x] 6.1 `docs/PHASE4-PANE-READINESS.md` §11: what sufficed, the three widenings,
      the node-budget measurement, and the itemised out-of-scope list.
- [x] 6.2 `docs/ARCHITECTURE.md`: ADR for the tint/fill roles, the published
      review section, and why the native renderer was not refactored.
- [x] 6.3 `CLAUDE.md`: the new capability, the new node and token, and the port's
      measurement. `docs/CONFIG.md` needed no edit — it lists config *files* and
      never enumerates the capability vocabulary.
- [x] 6.4 `docs/PHASE6-TEARDOWN-READINESS.md`: the code-review row stays blocked,
      with the reason updated to "reproduced in part, native pane still drawn".

## 7. Full verification

- [x] 7.1 `cargo fmt --all -- --check`
- [x] 7.2 `cargo clippy --all-targets --features plugins -- -D warnings` and
      `cargo clippy --all-targets -- -D warnings`
- [x] 7.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 7.4 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and
      `… --all --features plugins`
- [x] 7.5 `./scripts/dev/lint-luau.sh`, `rumdl check .`
- [x] 7.6 `cargo tree --edges normal | grep -c mlua` is 0

## 8. Not planned, found while doing it

- [x] 8.1 `tests/global_search_pane_gap.rs`: §10's *bottom-anchored region* probe
      listed a node named `Fill` as a closure, and this port added one — but this
      fill is an inline run whose width is a *line's* residue, so it anchors
      nothing vertically. The probe now asks the tree whether the fill it found is
      inlineable rather than trusting its name, and §11 records the correction.
      Verify: `cargo nextest run -E 'binary(global_search_pane_gap)'`.
