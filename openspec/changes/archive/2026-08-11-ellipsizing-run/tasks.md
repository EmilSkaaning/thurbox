# Tasks

## 1. The declaration

- [x] `src/session/view_tree.rs`: `TextStyle::ellipsize`, documented as the one field
      that is neither a colour nor an attribute — what happens when the line runs out.
- [x] `src/plugin/view.rs`: convert it with the same "only a literal `true`" rule the
      emphases use; a test that `false`, a string and a number all leave it off.
- [x] `src/plugin/capabilities.rs`: accept it in the style **table** only (the
      positional form is full), i.e. one more key in `apply_style_arg`.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'test(view) or test(capabilities)'`.

## 2. The kernel resolves it

- [x] `src/ui/plugin_pane.rs`: in the line resolution, give fixed runs their width,
      hand the remainder to consecutive yielding runs as **one** budget, and truncate
      with `ui::truncate_ellipsis` — the function the native panes fit with.
- [x] `src/ui/plugin_pane.rs`: a yielding run is resolved **before** a fill, since a
      fill is the line's residue.
- [x] Unit tests in `src/ui/plugin_pane.rs`: a line that fits is untouched; one that
      overflows keeps its trailing marker; several yielding runs are cut as one piece
      with a single ellipsis; a line with none clips as before.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'test(plugin_pane)'`.

## 3. The native tasks pane declares it instead of fitting

- [x] `src/ui/tasks_panel.rs`: `task_rows` loses its `width` argument and its
      `truncate_ellipsis` call; `tasks_tree` marks the title runs `ellipsize`; the
      marker and the glyph stay fixed. Update the module's own tests.
- [x] `src/app/view.rs`: the one call site drops the width.
- [x] Verify: `cargo check --all` and the pane's own unit tests.

## 4. The plugin declares it, and the divergence retires

- [x] `src/plugin/bundled/tasks/init.luau`: the title runs carry `ellipsize`, with the
      comment explaining that the pane's *width* is still not known to it.
- [x] `tests/bundled_tasks_panel.rs`: replace
      `a_title_wider_than_the_column_is_fitted_by_the_kernel_only` with an equality at
      the narrow width, and rewrite the module note (the divergence list is now
      empty).
- [x] `tests/view_tree_record/mod.rs`: print the new fact when it is set.
- [x] Re-record: `INSTA_UPDATE=always cargo test --test bundled_tasks_panel`, then
      **read the diff** and confirm it is one word per title run.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_tasks_panel)'`.

## 5. The gates re-verdict themselves

- [x] `tests/tasks_pane_input_gap.rs`: `no-ellipsizing-clip` is **closed**; the
      module note's "one vocabulary row is left" is no longer true.
- [x] `tests/automations_pane_handover_gap.rs`: `no-fitted-name` stays blocked but its
      probe narrows to the reason that is left — `resolve_rows` still fits the name —
      since "the catalogue cannot say it" has stopped being true.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`.

## 6. Documentation

- [x] `src/plugin/bundled/thurbox.d.luau`: the field on `TextStyle`, with what it
      means for a line.
- [x] `docs/ARCHITECTURE.md`: ADR-52.
- [x] `docs/PHASE4-PANE-READINESS.md` §27.
- [x] `CLAUDE.md`: the view-tree paragraph.
- [x] Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `./scripts/dev/lint-luau.sh`, `rumdl check .`.

## 7. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] `openspec validate ellipsizing-run --strict`
- [x] Hand-drive: a narrow tasks column shows a fitted title **and** its `⇄` marker,
      in both panes.
