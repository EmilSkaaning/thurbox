# Tasks — the Phase 6 teardown gate

## 1. The gate

- [x] 1.1 New `tests/teardown_gate.rs`: `Replacement { id, v1_capability, v2_home,
      ready, probe }` rows for the seven MIGRATION §4 capabilities, each probe a
      predicate over the source tree.
- [x] 1.2 `TeardownUnit { name, paths, requires }` rows: the v1 extension system
      as one unit (all five files, the `extensions/` tree, the two metadata keys),
      plus one unit per native pane requiring its bundled plugin directory.
- [x] 1.3 `every_listed_path_survives_until_its_unit_is_ready` — a missing path
      fails naming the unmet replacement ids.
- [x] 1.4 `recorded_verdicts_match_the_tree` — each `ready` equals its probe,
      failing with the row and the drift direction.
- [x] 1.5 `readiness_is_derived_from_the_verdicts` — a pure `blockers()` over the
      table: today's rows report unsafe naming all thirteen blockers, a synthetic
      all-ready table reports permitted.
- [x] 1.6 `inventory_is_well_formed` — no unit has an empty requirement set, and
      every required id resolves to a replacement row.

**Verify:** `cargo nextest run --test teardown_gate` in both feature
configurations.

## 2. Close the hook payload drift hole

- [x] 2.1 Hoist `materialize_source`'s filename→constant list into
      `pub(crate) const EMBEDDED_ASSETS: &[(&str, &str)]` and have the writer
      iterate it (no behaviour change).
- [x] 2.2 Extend `remote_assets_stay_in_sync_with_embedded_manifest` to resolve
      each manifest wiring's `source` through `EMBEDDED_ASSETS` and assert
      byte-identical payloads against the remote table.
- [x] 2.3 Assert every wiring's source resolves, so a manifest row pointed at an
      unembedded file fails rather than skipping.

**Verify:** `cargo nextest run -E 'test(remote_assets) + test(builtin_hooks)'`

## 3. Documentation

- [x] 3.1 `CLAUDE.md` "Architecture Enforcement": add the gate with its one-line
      purpose.
- [x] 3.2 `docs/ARCHITECTURE.md` ADR-23: record why the teardown inventory is
      executable and what the two tables mean.
- [x] 3.3 `docs/PHASE6-TEARDOWN-READINESS.md`: the prose companion — the verdicts
      with their evidence and the worklist that unblocks them — indexed in
      `docs/README.md`, mirroring the Phase 4 readiness audit.

## 4. Verification

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --all-targets --features plugins -- -D warnings` and the
      default-feature run.
- [x] 4.3 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 4.4 `cargo nextest run --all` and `--all --features plugins`
- [x] 4.5 `./scripts/dev/lint-luau.sh`, `rumdl check .`
- [x] 4.6 `cargo tree --edges normal | grep -c mlua` is 0.
