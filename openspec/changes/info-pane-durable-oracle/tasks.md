# Tasks

## 1. A legible, exhaustive recording of a view tree

- [x] `tests/bundled_info_panel.rs`: add a `record` module holding the
      line-per-node renderer — one line per node with indentation for depth, the
      node kind, its content, and only the non-default style facts. Match every
      `ViewNode` variant and bind every field by name; match `TextStyle` the same
      way. **No `..` rest pattern and no wildcard arm**, with a comment saying
      that this is what makes a field added to the IR fail to compile here rather
      than silently leave the oracle.
- [x] Verify the exhaustiveness claim by hand: temporarily add a field to
      `TextStyle`, confirm `cargo test --features plugins --test
      bundled_info_panel` fails to compile, then revert. Record the compiler's
      message in the final report.

## 2. Record the native pane, and check the recording is the native pane's

- [x] `tests/bundled_info_panel.rs`: in the existing per-case loop, keep the
      differential assertion (`plugin == native`) and add a recorded one
      (`native == snapshot`) using `insta::assert_snapshot!` with the case name as
      the snapshot name, so a failure names the case.
- [x] Generate the snapshots from the **native** builder with
      `INSTA_UPDATE=always cargo test --features plugins --test
      bundled_info_panel`, then read each one and confirm it is the info panel
      (sections, gauges, the session's fields) rather than a loading or error
      pane.
- [x] Document in the file's header why both edges are asserted while both sides
      exist, and what the surviving assertion becomes after the handover.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --features plugins -E
      'test(bundled_info_panel)'`, and `git status` shows no `.snap.new`.

## 3. Prove the new oracle can fail

- [x] Perturb `src/plugin/bundled/info-panel/init.luau` (change one style token),
      run the test, and confirm the *recorded* assertion fails and names the case
      — not only the differential one. Revert; record the observed failure in the
      final report.
- [x] Confirm the reverse direction too: perturb the native builder and confirm
      the recorded assertion fails, which is what makes the recording a statement
      about the native pane rather than a copy of it. Revert.

## 4. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` (unchanged: the
      info panel's row must still be blocked, since this change hands nothing over)
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] `openspec validate info-pane-durable-oracle --strict`
