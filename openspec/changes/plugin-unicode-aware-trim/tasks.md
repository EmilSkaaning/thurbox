# Tasks

## 1. The binding

- [x] 1.1 `thurbox.trim(s)` in `plugin::capabilities::build_module_table`, ungated,
      beside `ui` and before the table is frozen; the body is `str::trim`, not a
      hand-rolled whitespace set.
- [x] 1.2 Declare it in `src/plugin/bundled/thurbox.d.luau` on the `Thurbox` type, with
      the reason it exists.
- [x] 1.3 A property test: for ASCII whitespace, the Unicode separators Luau's `%s`
      cannot see, and a control character that must **not** be trimmed, the binding's
      answer is `str::trim`'s answer.
- [x] 1.4 A test that a plugin declaring **no** capabilities still has it, and that the
      capability vocabulary is unchanged.

## 2. The pane uses it

- [x] 2.1 The bundled session-list plugin trims with `thurbox.trim` instead of
      `string.match(activity, "^%s*(.-)%s*$")`.

## 3. The divergence inverts

- [x] 3.1 `non_ascii_whitespace_is_trimmed_by_the_kernel_only` becomes an equality under
      a name that says so, keeping a guard that the padded fixture still differs from its
      trimmed form.
- [x] 3.2 Flip `non-ascii-whitespace-is-the-kernels-trim` to closed in
      `tests/session_list_pane_handover_gap.rs`, with a probe that re-derives both halves
      — the binding exists and the plugin uses it.

## 4. Documentation

- [x] 4.1 ADR: why the predicate rather than the answer, what Luau's `%s` actually is,
      and that no capability was added.
- [x] 4.2 `docs/PHASE4-PANE-READINESS.md` section; `CLAUDE.md`'s plugin paragraph names
      the helper.

## 5. Verification

- [x] 5.1 `cargo fmt --all -- --check`
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` and the `--no-default-features` form
- [x] 5.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 5.4 `cargo nextest run --all` and the `--no-default-features` form
- [x] 5.5 `cargo test --test teardown_gate`, `--test architecture_rules`
- [x] 5.6 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] 5.7 Hand-drive: a session whose agent reports an activity padded with a no-break
      space renders identically in the native pane and in the bundled plugin's pane.
