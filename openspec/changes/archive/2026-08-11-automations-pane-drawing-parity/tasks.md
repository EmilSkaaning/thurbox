# Tasks

## 1. The native pane stops fitting its name

- [x] `src/ui/automations_panel.rs`: `resolve_rows` loses its `width` parameter and
      keeps the name whole; `row_prefix`/`row_tail` stay as they are.
- [x] `src/ui/automations_panel.rs`: `row_node` marks every run of the name
      `ellipsize`, with the marker and the summary tail left at their intrinsic
      widths; a second style constant for the matched run, since `MATCHED_STYLE` is
      also used where nothing yields.
- [x] No caller changes: the width came from `inner.width` inside the pane, so
      `src/app/view.rs` passes the same state it always did.
- [x] `src/ui/automations_panel.rs`: the module note and `resolve_rows`' doc stop
      describing a width step that no longer exists.
- [x] Verify: `cargo check --all && cargo check --all --no-default-features`.

## 2. The native pane draws the shared frame

- [x] `src/ui/automations_panel.rs`: `render_automations_pane` builds
      `super::focus_block(" Automations ", state.focus)`; the `Block`/`Borders`/`Style`
      imports go if nothing else needs them.
- [x] `src/ui/automations_panel.rs`: `legacy_render` (the retained pre-port oracle)
      builds the same frame, so the cell comparison stays a claim about the rows.
- [x] `src/app/snapshots/thurbox__app__acceptance__empty_welcome_screen_renders.snap`:
      accept the band's rounded corners.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(thurbox) and test(automations)'` plus the acceptance snapshot test.

## 3. The plugin declares the same fit

- [x] `src/plugin/bundled/automations/init.luau`: a `fittedText` helper using the
      style-table form with `ellipsize = true`; the name's runs go through it, the
      marker and the summary do not. Header comment: nothing here knows a width, and
      the declaration is what keeps the summary tail.
- [x] `src/plugin/bundled/automations/plugin.toml`: note the fit among what the pane
      declares.
- [x] Verify: `./scripts/dev/lint-luau.sh`.

## 4. The recordings are regenerated from the native builder

- [x] `INSTA_UPDATE=always GIT_CONFIG_GLOBAL=/dev/null cargo test --test
      bundled_automations_panel` and review `git diff tests/snapshots/`.
- [x] Verify the diff is a multiset: every changed line is the same line plus
      `ellipsize`, and no line appears or disappears
      (`git diff -U0 tests/snapshots/` filtered to `^[-+]` and compared as sorted
      multisets after stripping the flag).
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_automations_panel)'`.

## 5. The oracle's enumerated divergence becomes its opposite

- [x] `tests/bundled_automations_panel.rs`: replace
      `a_name_wider_than_the_column_is_fitted_by_the_kernel_only` with a test that
      both panes paint the **same** frame at a narrow width, ellipsis and summary tail
      included, and that the row is genuinely fitted there.
- [x] `tests/bundled_automations_panel.rs`: `Case::native_rows`/`native_tree` lose the
      width argument; `the_comparison_size_adjusts_nothing` becomes an assertion that
      no width is taken at all, and the module note drops the fitted-name divergence.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_automations_panel)'`.

## 6. The gap gate

- [x] `tests/automations_pane_handover_gap.rs`: `no-fitted-name` is re-verdicted
      `blocked: false` with a probe reading that the native pane no longer fits, and
      its `stands` records both halves landing together.
- [x] `tests/automations_pane_handover_gap.rs`:
      `the_verdict_is_derived_from_the_blockers` asserts no `Vocabulary` row is
      outstanding, with the reason a new one would need naming.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test
      automations_pane_handover_gap` and `--test teardown_gate`.

## 7. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-55 — the two convergences, and why each runs toward
      the kernel's answer.
- [x] `docs/PHASE4-PANE-READINESS.md` §30.
- [x] Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `rumdl check .`.

## 8. Full verification, and driving it

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test architecture_rules`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] `openspec validate automations-pane-drawing-parity --strict`
- [x] Hand-drive: at 150 columns with the bundled pane shown beside the native band,
      the two are cell-identical including a name long enough to be cut; the band's
      corners are rounded and match ` Sessions `; opening the central automation
      editor gives the band an accent border.
