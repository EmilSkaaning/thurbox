# Tasks

## 1. One recorder, shared

- [x] Create `tests/view_tree_record/mod.rs` holding the line-per-node renderer
      currently private to `tests/bundled_info_panel.rs`: every `ViewNode`
      variant and every `TextStyle` field destructured by name, no `..` rest
      pattern, no wildcard arm. Carry over the module note explaining that this
      is what makes a field added to the IR a compile error rather than a silent
      hole, and add why it is shared while the gates' source-reading helpers are
      duplicated.
- [x] `tests/bundled_info_panel.rs`: replace the private `record` module with
      `mod view_tree_record;` and a `use` of it, leaving the assertions and the
      checked-in snapshots byte-identical.
- [x] Verify the exhaustiveness claim by hand. Adding a field to `TextStyle`
      cannot show it: six struct literals under `src/` fail first with E0063 and
      the library never compiles, so the test crate is not reached. The
      equivalent, and the property actually relied on, is that the pattern has no
      `..` — so **removing** a field from the destructure must be an error.
      Confirm E0027 naming the field, at the one location, then revert.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --features plugins -E
      'test(bundled_info_panel)'` and `git status` shows no `.snap.new` — the
      info panel's ten recordings must not move when their renderer relocates.

## 2. Record the session list

- [x] `tests/bundled_session_list.rs`: `mod view_tree_record;`, and in the
      existing per-case loop of `the_plugin_builds_the_native_panes_view_tree`
      add the recorded edge (`native == snapshot`) beside the differential one,
      named by the case, plus the conjunction (`plugin == snapshot`).
- [x] Leave the enumerated-divergence tests unrecorded (they assert inequality),
      and say so in the file's header rather than only in `design.md`.
- [x] Generate with `INSTA_UPDATE=always cargo test --features plugins --test
      bundled_session_list`, then read each snapshot and confirm it is the
      session list — group header, status glyph, the spinner's motion node with
      its rate and frames, the nested prefix, the selection fill.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --features plugins -E
      'test(bundled_session_list)'`; `git status` shows no `.snap.new`.

## 3. Record the automations pane

- [x] `tests/bundled_automations_panel.rs`: the same two edges in its per-case
      loop, named by the case.
- [x] Generate, then read each snapshot and confirm it is the automations pane —
      the composed summary's three parts, the enabled marker, the cursor's row.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --features plugins -E
      'test(bundled_automations_panel)'`; no `.snap.new`.

## 4. A readable failure

- [x] `tests/view_tree_record/mod.rs`: add `assert_matches`, which compares two
      recordings line by line and names the first line that moved — because the
      first perturbation run failed through `assert_eq!` on two trees, printing
      the structural dump the recording exists to replace.
- [x] Assert in the order recording → legible → exact in all three pane oracles,
      including `tests/bundled_info_panel.rs`, so one shape covers every pane.

## 5. Prove both oracles can fail, in both directions

- [x] Perturb `src/plugin/bundled/session-list/init.luau` (one style token), run
      the test, confirm the *recorded* conjunction fails and names the case.
      Revert; record the observed diff.
- [x] Perturb `ui::project_list`'s presentation step, confirm the recorded edge
      fails — which is what makes the recording a statement about the native pane
      rather than a copy of it. Revert; record the diff.
- [x] The same two perturbations for the automations pane
      (`src/plugin/bundled/automations/init.luau`, then
      `ui::automations_panel::row_summary`).

## 6. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — unchanged;
      both rows must still be blocked, since this change hands nothing over
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
- [x] `openspec validate left-column-pane-oracles --strict`
