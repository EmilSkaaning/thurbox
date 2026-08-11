# Tasks

## 1. The machine-checked record

- [x] `tests/global_search_pane_gap.rs` (new, no Cargo feature — it reads the
  source tree, so it runs and means the same thing in both configurations,
  mirroring `tests/teardown_gate.rs`): a `BLOCKERS` table of one row per missing
  host power, each with its recorded verdict, a probe that re-derives it from the
  declaration it reads, and whether it is structural or vocabulary. Tests:
  `recorded_blockers_match_the_tree`, `the_verdict_is_derived_from_the_blockers`
  (a vocabulary widening alone must not read as portable),
  `no_bundled_plugin_claims_global_search`,
  `the_search_verdict_crosses_outward_but_no_query_comes_back` (the outward half
  of the cross-pane evidence, asserted separately from the verdict), and
  `the_record_is_well_formed`.
  Verify: `cargo nextest run --test global_search_pane_gap`
- [x] Confirm the record and the teardown gate agree rather than overlap.
  Verify: `cargo nextest run --test teardown_gate`

## 2. The written record

- [x] `docs/PHASE4-PANE-READINESS.md`: the port's own section — the surface is a
  *mode*, the four structural blockers with their evidence, the four vocabulary
  gaps left open on purpose, the rejected rendering-only port, and the provider
  shape named but not designed.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the pane table's global-search row
  points at that section, so "absent" reads as a decision rather than a to-do.
  Verify: `rumdl check .`

## 3. Nothing in `src/` changes

- [x] `git diff --stat src/` is empty: no capability, no view node, no style
  token, no pane slot, no binding, and no touch to the native strip.
  Verify: `git status --short src/`
- [x] The insta acceptance snapshots have not moved, which follows from the above
  but is what a reviewer will want asserted.
  Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E 'test(acceptance)'`

## 4. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` (≥ 2154, 0 failed)
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
  (≥ 2489, 0 failed)
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `./scripts/dev/lint-luau.sh` ; `rumdl check .`
