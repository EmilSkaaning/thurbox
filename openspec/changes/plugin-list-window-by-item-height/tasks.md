# Tasks

## 1. Generalise the windowing rule

- [x] 1.1 `src/ui/mod.rs`: add `visible_item_window(total, item_rows, selected, height)`,
  the row-measured rule, with the two clauses unit heights cannot reach documented as
  such.
- [x] 1.2 `src/ui/mod.rs`: `visible_window` becomes a wrapper over it (`|_| 1`), so the
  three uniform callers keep their signature and there is one implementation.
- [x] 1.3 `src/ui/mod.rs` tests: `the_general_rule_reduces_to_the_uniform_one` walks every
  `(total, selected, height)` triple in a range and requires the identical pair; plus the
  two cases only variable heights reach (a child taller than the pane, a tall neighbour
  above the cursor).

## 2. Window a plugin list in rows

- [x] 2.1 `src/ui/plugin_pane.rs`: the `ViewNode::List` arm measures each child once with
  `height_of` — only when the list declares a cursor or a track, so a list that does
  neither measures nothing — and resolves the window through `visible_item_window`.
- [x] 2.2 `src/ui/plugin_pane.rs`: the declared track's overflow test, content length and
  thumb position are row quantities.
- [x] 2.3 `src/ui/plugin_pane.rs` tests: a list of two-line children scrolls to a cursor
  past the fold; a click on either line of a two-line child reports that child's index; a
  track appears for children that overflow only in rows; a list of one-line children is
  unchanged.

## 3. Re-verdict the gate

- [x] 3.1 `tests/session_list_pane_handover_gap.rs`: `the-window-is-the-list-widgets`
  stays **blocked**; `plugin_window_is_the_shared_rule` names `visible_item_window`, and
  the row's `stands` records which half moved.
- [x] 3.2 `cargo test --test session_list_pane_handover_gap --test teardown_gate
  --test architecture_rules`

## 4. Record it

- [x] 4.1 `docs/ARCHITECTURE.md`: an ADR for the row-measured window and the rejected
  sticky rule.
- [x] 4.2 `docs/PHASE4-PANE-READINESS.md`: the section the session-list gate points at.

## 5. Verify

- [x] 5.1 `cargo fmt --all -- --check`
- [x] 5.2 `cargo clippy --all-targets -- -D warnings`
- [x] 5.3 `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] 5.4 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 5.5 `cargo nextest run --all` — no snapshot moves
- [x] 5.6 `cargo nextest run --all --no-default-features`
- [x] 5.7 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] 5.8 Hand-drive: `scripts/dev/sandbox.sh --fresh --show file-viewer` boots, the seated
  file-viewer plugin pane draws its frame, and the session list, its repo-group header and
  the pending-spawn placeholder render unchanged. **Not done by hand: scrolling that tree
  past the fold** — the sandbox session stayed in `Setting up…` (the new-session flow's
  `git fetch` against this repo never returned), so no file tree was ever populated. That
  path is covered by `tests/bundled_file_viewer.rs`'s frame equality at a size where the
  pane scrolls, which is the only list in the tree that declares a track.
