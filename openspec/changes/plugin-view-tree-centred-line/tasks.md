# Tasks

## 1. The node

- [x] 1.1 `src/session/view_tree.rs`: `ViewNode::Center(Vec<ViewNode>)`, with `children`,
  `kind_name`, `first_non_inlineable` (children as a line's; itself not inlineable) and a
  constructor.
- [x] 1.2 `src/session/view_tree.rs` tests: it names itself, its runs count toward the
  bounds, its children follow the line rule, and it is refused inside a line.

## 2. The paint

- [x] 2.1 `src/ui/plugin_pane.rs`: `height_of` is one row, `inline_width` is zero (it
  never appears in a line), and `paint` draws its spans through the same
  `Alignment::Center` `ui::project_list::render_empty_sessions` uses.
- [x] 2.2 `src/ui/plugin_pane.rs` tests: a centred run sits in the middle, the odd column
  falls where ratatui puts it, a full-width row is unchanged, and an overflowing one is
  clipped at one row.

## 3. The constructor

- [x] 3.1 `src/plugin/capabilities.rs`: `center` joins the `row`/`line`/`paragraph`/
  `column` constructor loop. No `Capability` variant, no module binding.
- [x] 3.2 `src/plugin/view.rs`: convert `"center"`, refusing a non-inlineable child by
  the same rule a line does.
- [x] 3.3 `src/plugin/bundled/thurbox.d.luau`: declare `ui.center`.
- [x] 3.4 `src/plugin/view.rs` tests: it converts, a bad child is refused naming the kind,
  and a centred node inside a line is refused.

## 4. Re-verdict the gate

- [x] 4.1 `tests/session_list_pane_handover_gap.rs`: `no-centred-line` → **met**, on the
  rule the seat rows already use; probe asks whether the vocabulary is still missing;
  `stands` records that neither pane adopted it.
- [x] 4.2 `the_verdict_is_derived_from_the_blockers` still refuses the handover and still
  has outstanding vocabulary rows.
- [x] 4.3 `tests/bundled_session_list.rs`'s empty-pane divergence still passes unchanged in
  substance; its doc comment is rewritten to separate the gap (closed) from the divergence
  (not), so the test now says what fails if one pane adopts the node and the other does
  not.

## 4b. Not verified by hand, and why

- [x] 4b.1 No hand-drive. Nothing on screen draws a centred node: the native empty state
  is a `Paragraph` and the bundled plugin was deliberately not changed, so there is no
  frame a `tmux send-keys` could show that differs from yesterday's. The placement is
  pinned by `the_odd_column_falls_where_the_kernels_own_centring_puts_it`, which asserts
  the rendered columns against the fill-based approximation it replaces.

## 5. Record it

- [x] 5.1 `docs/ARCHITECTURE.md`: an ADR with the three rejected shapes.
- [x] 5.2 `docs/PHASE4-PANE-READINESS.md`: the section the gate points at.

## 6. Verify

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --all-targets -- -D warnings`
- [x] 6.3 `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] 6.4 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 6.5 `cargo nextest run --all` — no snapshot moves
- [x] 6.6 `cargo nextest run --all --no-default-features`
- [x] 6.7 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
