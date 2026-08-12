# Tasks

## 1. The chrome shape

- [x] 1.1 `src/app/mod.rs`: `PaneChrome::StatusDots { statuses, spinner_frame }`, with the
  doc stating that it is the first shape that subtracts nothing.
- [x] 1.2 `src/app/mod.rs`: `App::pane_chrome` answers it for `KeyContext::SessionList`,
  from the sessions in render order and `App::spinner_frame`; `None` with no sessions, as
  the native pane draws none.
- [x] 1.3 `src/app/view.rs`: `plugin_pane_chrome` passes it through unconditionally (it
  follows the pane, not focus — the native dots do not follow focus either).

Verify: `cargo check --all-targets`.

## 2. The paint

- [x] 2.1 `src/app/view.rs`: `paint_plugin_pane` resolves the dots onto the block's top
  title, right-aligned, through `ui::status_glyph` / `ui::status_color`, before the block
  is rendered — so no split changes and the content area is untouched.
- [x] 2.2 `src/app/view.rs`: after `render_tree_rows`, draw `ui::draw_clipped_indicators`
  from the counts the painter returned, for every pane the host paints.

Verify: `cargo clippy --all-targets -- -D warnings`.

## 3. Tests

- [x] 3.1 `src/app/acceptance.rs`: a seated pane declaring the session-list keyboard draws
  one dot per session on its top border, in the session's status colour.
- [x] 3.2 `src/app/acceptance.rs`: no sessions ⇒ no dots.
- [x] 3.3 `src/app/acceptance.rs`: the dots cost the pane no content row — the same tree
  paints into the same cells with and without them.
- [x] 3.4 `src/app/acceptance.rs`: a seated pane whose list overflows draws `▲ N` / `▼ N`
  on its borders, and one that fits draws neither.

Verify: `cargo nextest run -E 'test(border) or test(clipped) or test(status_dots)'`.

## 4. Re-verdict the gate

- [x] 4.1 `tests/session_list_pane_handover_gap.rs`: `no-pane-chrome` → **met**; the probe
  asks whether the shape and the indicator are still missing, and asserts the two halves
  that make "closed" mean what it says — nothing in `PaneDecl` declares chrome, and the
  native pane is still the one drawing its own.
- [x] 4.2 `the_verdict_is_derived_from_the_blockers` still refuses the handover, with
  `no-pending-spawn-row` the one row left.

Verify: `cargo test --test session_list_pane_handover_gap`.

## 5. Docs

- [x] 5.1 `docs/ARCHITECTURE.md`: ADR for the third chrome shape and for the indicators
  becoming the host's.
- [x] 5.2 `docs/PHASE4-PANE-READINESS.md`: record the row closing.

Verify: `rumdl check .`

## 6. Full gate

- [x] 6.1 `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo clippy --all-targets --no-default-features -- -D warnings`;
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`.
- [x] 6.2 `cargo nextest run --all` and `cargo nextest run --all --no-default-features`,
  with `commit.gpgsign` forced off.
- [x] 6.3 `cargo test --test teardown_gate`; `cargo test --test architecture_rules`;
  `./scripts/dev/lint-luau.sh`; `./scripts/dev/lint-workflows.sh`; `rumdl check .`
- [x] 6.4 Hand-drive it in `scripts/dev/sandbox.sh --fresh`.
