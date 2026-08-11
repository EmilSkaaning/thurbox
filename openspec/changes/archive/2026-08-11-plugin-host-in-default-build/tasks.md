# Tasks — the plugin host in the default build

## 1. Measure, before changing anything that depends on the answer

- [x] 1.1 Record the baseline release build: `cargo build --release --bins` with
  `default = []`, capturing wall time and both binary sizes.
  Verify: `stat -c "%n %s" target/release/thurbox target/release/thurbox-cli`
- [x] 1.2 Establish what the runtime is compiled from — `mlua` →`mlua-sys` →
  `luau0-src` — and whether it is C or C++.
  Verify: `grep -n 'std(\|cpp(' ~/.cargo/registry/src/*/luau0-src-*/src/lib.rs`
- [x] 1.3 Cross-build `x86_64-unknown-linux-musl` against a C++-capable
  `musl-cross` toolchain, pointed at through the same four variables `cross`'s
  image sets (`CC_`/`CXX_`/`AR_`/`CARGO_TARGET_*_LINKER`), and confirm the runtime
  linked.
  Verify: `file target/x86_64-unknown-linux-musl/release/thurbox` and
  `strings -a … | grep -c luau`
- [x] 1.4 Cross-build `x86_64-pc-windows-gnu` with mingw `g++` as the nearest
  available proxy for the MSVC target, and record what it does and does not prove.
  Verify: `cargo build --release --bins --target x86_64-pc-windows-gnu`
- [x] 1.5 Confirm what `cross`'s musl image supplies (`docker/musl.sh`,
  `Dockerfile.x86_64-unknown-linux-musl`) and audit Luau's compiled sources for
  C++17 library features its GCC 9.2 would lack.
  Verify: `grep -rn 'from_chars\|<charconv>\|<filesystem>\|starts_with'` over
  `luau/{Ast,Compiler,VM,Config}`
- [x] 1.6 Record every result in `design.md`, naming the two targets that stay
  unverified rather than assuming them.

## 2. The manifest and the MSRV

- [x] 2.1 `Cargo.toml`: `default = ["plugins"]`, `rust-version = "1.88"`, and
  rewrite both comments — the per-feature-MSRV note describes a workaround the
  manifest no longer needs, and the `[features]` note says the runtime is absent
  from stable builds.
- [x] 2.2 `clippy.toml`: `msrv = "1.88"` and correct the comment naming `ratatui`
  as the binding constraint.
  Verify: `cargo clippy --all-targets -- -D warnings`
- [x] 2.3 Correct the four documents claiming MSRV 1.75 (`CLAUDE.md`,
  `README.md` — which also gains the C++ toolchain requirement —
  `CONTRIBUTING.md`, `openspec/config.yaml`).
  Verify: `grep -rn '1\.75' CLAUDE.md README.md CONTRIBUTING.md openspec/config.yaml`

## 3. Invert the CI assertion and re-aim the job around it

- [x] 3.1 `.github/workflows/ci.yml`: the `plugins` job's final step asserts the
  default dependency tree **contains** `mlua`. Rewrite it, do not delete it.
- [x] 3.2 Replace the job's now-duplicated `--features plugins` clippy and test
  runs with the configuration nothing else covers: `--no-default-features` clippy
  and `nextest`. Keep the pinned 1.88 toolchain (now a real MSRV floor check) and
  the Luau type-check.
- [x] 3.3 Retitle the job, keep the id `plugins` (`all-checks.needs` and branch
  protection name it), and update both comments to say what it now verifies.
  Verify: `./scripts/dev/lint-workflows.sh` and, locally, the two commands the job
  runs — `cargo clippy --all-targets --no-default-features -- -D warnings` and
  `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`

## 4. Amend the release invariant, in the spec first

- [x] 4.1 Delta spec: `REMOVED` the requirement that `cd.yml` never builds with
  the plugin feature, with **Reason** and **Migration**; `ADDED` the requirement
  that it never builds without the runtime.
  File: `openspec/changes/plugin-host-in-default-build/specs/release/workflow-invariants/spec.md`
- [x] 4.2 `scripts/dev/lint-workflows.sh`: rename
  `invariant_no_plugin_feature` → `invariant_keeps_plugin_runtime`, reject
  `--no-default-features` and a `default = [` manifest edit, and rewrite the
  header block and the file's four-invariant summary.
- [x] 4.3 `scripts/dev/lint-workflows.bats`: replace the five invariant-2 cases
  with the reversed set, including one asserting that an explicit
  `--features plugins` / `--all-features` now **passes**.
  Verify: `bats scripts/dev/lint-workflows.bats` and `./scripts/dev/lint-workflows.sh`
- [x] 4.4 `CLAUDE.md`'s summary of the four invariants: state the new direction and
  that it reversed, not just the new rule.

## 5. Keep the fresh launch looking like the launch before it

- [x] 5.1 `src/plugin/bundled/hello/plugin.toml`: `default_visible = false`, with
  the reason.
- [x] 5.2 New `tests/bundled_manifests.rs` holding the rule over the whole bundled
  set, with the empty `PANES_DRAWN_IN_A_NATIVE_PANES_PLACE` allowlist a handover
  will use, plus a non-vacuity test.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test bundled_manifests`
- [x] 5.3 `scripts/dev/sandbox.sh`: drop the `--show needs --plugins` guard (the
  default build has the host), and correct the header's claims about `--plugins`
  and about which panes ship hidden.
  Verify: `shellcheck scripts/dev/sandbox.sh`

## 6. Flip the teardown gate's build condition honestly

- [x] 6.1 Delta spec: `MODIFIED` the handover requirement so the build condition
  stays checked once satisfied, with the scenario that names it.
  File: `openspec/changes/plugin-host-in-default-build/specs/migration/teardown/spec.md`
- [x] 6.2 `tests/teardown_gate.rs`: leave `plugin_host_reaches_the_installed_build`
  alone; rewrite `a_pane_drawn_only_by_a_gated_build_is_not_handed_over` into
  `the_build_condition_holds_and_still_gates_a_handover`, asserting the condition
  holds, that the pure rule is unchanged, and that each pane row is blocked by its
  own pane-level reason.
- [x] 6.3 Correct the module note and the `pane()` doc comment, both of which say
  the runtime is optional and the release workflow may not enable it.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`

## 7. Record the decision

- [x] 7.1 `docs/ARCHITECTURE.md`: ADR-40, with the per-target table, the size and
  build-time deltas, and the rejected alternatives (including the runtime
  `[features]` flag the earlier prose design set expected).
- [x] 7.2 Amend ADR-37's rejected alternative to point at ADR-40 and correct its
  two factual slips (C, not C++; `aarch64-apple-darwin` is native, not
  cross-compiled).
- [x] 7.3 `docs/PHASE6-TEARDOWN-READINESS.md` §3 and §4 step 7: the third handover
  condition now holds and stays checked; the worklist step is done, with the
  unverified targets named.
- [x] 7.4 `docs/PHASE4-PANE-READINESS.md` §14: a resolution note under the "blocker
  is the build" table rather than a rewrite of the record.
- [x] 7.5 `docs/CONSTITUTION.md` and `docs/KERNEL-BOUNDARY.md`: the four host state
  fields are no longer "gated", and the `plugin` module is no longer "behind" a
  feature.
  Verify: `rumdl check .`

## 8. The MSRV bump's own fallout

Raising `msrv` to 1.88 un-suppresses `clippy::manual_is_multiple_of`
(`u64::is_multiple_of` stabilised in 1.87), which fails eight `% N == 0` tick
cadence checks under `-D warnings`. Found by running the verification, not
predicted — and it is the only reason this change touches `src/` beyond one
manifest line.

- [x] 8.1 Rewrite the eight sites as `is_multiple_of` in `src/app/mod.rs`
  (six: the automation/task refresh cadence, the two perf-window gates, the
  metrics/git/config-reload cadences, the hook version check) and
  `src/app/automation.rs` (one, negated).
  Verify: `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy --all-targets --features plugins -- -D warnings`

## 9. Full verification

- [x] 9.1 `openspec validate plugin-host-in-default-build --strict`
- [x] 9.2 `cargo fmt --all -- --check`
- [x] 9.3 `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] 9.4 `cargo clippy --all-targets -- -D warnings`
- [x] 9.5 `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] 9.6 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 9.7 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] 9.8 `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] 9.9 `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
- [x] 9.10 `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
  `bats scripts/dev/lint-workflows.bats`, `rumdl check .`, `shellcheck`
- [x] 9.11 Drive it by hand: `scripts/dev/sandbox.sh --fresh --show hello`,
  confirming a default build boots with the host, shows **no** plugin pane until
  asked, and shows one when asked.
- [x] 9.12 Prove both new gates non-vacuous: with `default = []` restored,
  `the_build_condition_holds_and_still_gates_a_handover` fails with its own
  message; with `hello`'s seed set back to `true`,
  `every_bundled_pane_seeds_hidden` fails naming the pane.
- [x] 9.13 `cargo deny check advisories` and `cargo deny check bans licenses
  sources`, since the runtime and its vendored Luau now enter the *default*
  dependency tree rather than only the `--all-features` one.
