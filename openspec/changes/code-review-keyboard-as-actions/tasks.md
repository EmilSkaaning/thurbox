# Tasks — the code review's keyboard as kernel actions

## 1. The keybinding vocabulary

- [x] 1.1 `src/session/keybindings.rs`: add `KeyContext::CodeReview` and
  `KeyContext::ReviewFiles`; add both to `KeyContext::pane_keyboards()` with the reason
  they are declarable (their surface is conditional, and the condition is the kernel's).
- [x] 1.2 `src/session/keybindings.rs`: add the 28 `Review*` and 11 `ReviewFiles*` variants
  to `Action`, and to `Action::all()` in the same order.
- [x] 1.3 `src/session/keybindings.rs`: `Action::label`, `Action::context` and
  `Action::default_chords_for` arms for all 39. Half-paging defaults to `d`/`u` (see
  design.md); no scoped default may shadow a global one.
- [x] 1.4 `src/session/keybindings.rs`: two `help_sections` entries — `Code review (when
  focused)` and `Review files (when focused)` — appended after `Terminal`.
- [x] 1.5 Tests in `src/session/keybindings.rs`: the two contexts round-trip as names; the
  review defaults introduce no conflict; the two contexts do not overlap.

Verify: `cargo nextest run -E 'test(keybindings)'`, `cargo nextest run -E
'test(macos_default_set_has_no_conflicts)'`, `cargo nextest run -E
'test(every_action_has_default_chord_and_context)'`.

## 2. Dispatch

- [x] 2.1 `src/app/code_review.rs`: `dispatch_code_review_action` and
  `dispatch_review_files_action`, one arm per action, calling the existing `cr_*` methods.
  `ReviewClose` clears a committed search when one is active, else closes the review.
- [x] 2.2 `src/app/key_handlers.rs`: route both families from
  `dispatch_scoped_pane_action`.
- [x] 2.3 `src/app/key_handlers.rs`: `focus_key_context` maps `InputFocus::CodeReview` and
  `InputFocus::ReviewFiles` to their contexts, falling back to `Global` while a sub-mode
  owns the keyboard (mirroring the file viewer's `search_active` arm).
- [x] 2.4 `src/app/mod.rs`: `App::focus_for_keyboard` and `App::pane_chrome` arms for both
  contexts (`pane_chrome` is `None` — the review's own chrome is not seat chrome).

Verify: `cargo nextest run -E 'test(code_review)'`.

## 3. Retire the captures

- [x] 3.1 `src/app/code_review.rs`: delete `handle_review_files_key` and
  `review_escape_chord`; shrink `handle_code_review_key` to
  `handle_code_review_submode_key` — target picker, compose box, find-while-typing only.
- [x] 3.2 `src/app/key_handlers.rs`: `handle_key` calls the one sub-mode capture; the
  comment says which sub-modes and why they are not actions.
- [x] 3.3 Acceptance coverage in `src/app/acceptance.rs`: the diff pane's keys still act
  through the lookup; a rebound review key fires; a global chord the capture used to
  swallow now fires; `d`/`u` half-page.

Verify: `cargo nextest run -E 'test(acceptance)'`, `cargo nextest run --all`.

## 4. Re-verdict the refusal

- [x] 4.1 `tests/code_review_pane_handover_gap.rs`: re-verdict
  `keys-are-a-capture-not-actions` met, with a probe deriving the contexts, the actions and
  the dispatch.
- [x] 4.2 Same file: re-verdict `no-review-write`, `no-retarget-operation`,
  `no-export-operation` and `no-cursor-write` met — each keeping an assertion that the
  capability it named is **still** absent.
- [x] 4.3 Same file: rewrite `the_reviews_keyboard_resolves_no_action` into the finding it
  became, and update the module doc's three findings and
  `the_verdict_is_derived_from_the_blockers`.
- [x] 4.4 Same file: narrow `no-in-pane-text-field` to the compose body and sharpen
  `no-anchored-overlay` with the anchor argument.

Verify: `cargo test --test code_review_pane_handover_gap`, `cargo test --test
teardown_gate`, `cargo test --test architecture_rules`.

## 5. Documentation

- [x] 5.1 `CLAUDE.md`: the code-review section records that the pane's keys are scoped
  rebindable actions in two contexts, and the two decided differences.
- [x] 5.2 `docs/PHASE4-PANE-READINESS.md` §20 and `docs/ARCHITECTURE.md` (new ADR): the
  route, the two differences, and the five rows that still refuse the handover.
- [x] 5.3 `docs/CONFIG.md`: the two new key contexts in the keybindings reference.

Verify: `rumdl check .`.

## 6. Full verification

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] 6.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 6.4 `cargo nextest run --all` and `cargo nextest run --all --no-default-features`
- [x] 6.5 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] 6.6 Hand-drive: `scripts/dev/sandbox.sh --fresh`, open a review, walk every key in
  both panes, rebind one in F1 and confirm it takes effect without a restart.
