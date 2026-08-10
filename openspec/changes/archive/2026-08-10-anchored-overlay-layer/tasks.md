# Tasks — the anchored overlay layer

## 1. The anchor spec and resolver

- [x] 1.1 Add `src/session/overlay.rs`: `Side`, `Align`, `CrossExtent`,
  `Overlay`, and `Overlay::place(target: Option<Rect>, clip: Rect) -> Rect`
  implementing clamp → prefer → flip → dock. Register it in
  `src/session/mod.rs`.
- [x] 1.2 Unit tests in the same file: each side, flip taken/refused, dock on
  the no-room and no-target paths, containment with the target swept across and
  past the clip, extent clamped to a smaller clip, stretch inset, the three
  alignments, alignment clamped at the clip edge.
- [x] 1.3 Confirm no `tests/architecture_rules.rs` change is required: the
  allowlist is per top-level module, and `session`'s empty allowance already
  governs this file.

Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(overlay)'` and
`GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --test architecture_rules`.

## 2. The per-pane overlay layer

- [x] 2.1 Add `src/ui/overlay.rs`: `OverlayLayer` with `place` and
  `into_hit_order` (topmost first). Register it in `src/ui/mod.rs`.
- [x] 2.2 Unit tests: two declarations report topmost-first; a layer with
  nothing placed is empty; a placed rect is contained in its clip.
- [x] 2.3 Confirm no `tests/architecture_rules.rs` change is required:
  `ui::overlay` references `session`, an edge `ui` already has.

Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(overlay)'`.

## 3. Port the code-review compose box

- [x] 3.1 In `src/ui/code_review.rs`, replace `render_compose_inline` with
  `compose_anchor` — an `Overlay` declaration placed through an `OverlayLayer`
  against the selected row's `RowHitbox` rect; add `overlay: Vec<Rect>` to
  `CodeReviewHits`.
- [x] 3.2 Unit tests in `src/ui/code_review.rs`: the pre-port formula kept as an
  oracle (`legacy_compose_rect`) and swept over every anchor row for pane heights
  3..23 and widths 2..60, the clamped degenerate case asserted separately, and an
  empty overlay list when nothing is composed.

Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(compose)'` and
`GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (no snapshot may move).

## 4. Overlay-first hit-testing

- [x] 4.1 Add `ClickAction::OverlayCapture` in `src/app/mod.rs`, record the
  review pane's overlay rects **before** its row targets in `src/app/view.rs`,
  and consume the click in `activate_click_target`.
- [x] 4.2 Acceptance test in `src/app/acceptance.rs`: with a compose box open, a
  click on a covered diff row leaves the selection where it was; the same click
  with the box cancelled selects that row.

Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run -E 'test(overlay)'`.

## 5. Docs

- [x] 5.1 `docs/ARCHITECTURE.md`: ADR-25, with the rejected alternatives and the
  one named behavioural divergence.
- [x] 5.2 `CLAUDE.md`: the `ui/` architecture bullet (base layer vs overlay
  layer) and the code-review comments bullet.
- [x] 5.3 `docs/PHASE4-PANE-READINESS.md` deliberately **not** touched — its
  five gaps are about what a plugin pane cannot express, and a floating element
  is not one of them.

Verify: `rumdl check .`

## 6. Full verification before commit

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] 6.3 `cargo clippy --all-targets -- -D warnings`
- [x] 6.4 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 6.5 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] 6.6 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] 6.7 `cargo tree --edges normal | grep -c mlua` → `0`
- [x] 6.8 `./scripts/dev/lint-luau.sh` and `rumdl check .`
