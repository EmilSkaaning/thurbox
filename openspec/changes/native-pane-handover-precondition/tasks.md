# Tasks

## 1. Tighten the pane-handover probe

- [ ] `tests/teardown_gate.rs`: add `plugin_host_reaches_the_installed_build(root)`,
      reading `Cargo.toml`'s `[features] default = [...]` for `plugins`, and make it
      the third conjunct of `pane()`'s probe. Update `pane()`'s doc comment to state
      all three conditions and why the third is global rather than per-pane.
- [ ] `tests/teardown_gate.rs`: add
      `a_pane_drawn_only_by_a_gated_build_is_not_handed_over`, asserting the probe
      answers unready for a pane whose plugin exists and whose native renderer is
      no longer drawn while the runtime is gated — the case the two-conjunct probe
      permitted — plus that the condition currently blocks every pane row.
- [ ] `tests/teardown_gate.rs`: update the module documentation so the recorded
      reason for the pane rows is the conjunction of three conditions.
- [ ] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate`
      and `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
      (the verdict must be identical in both, which is the point of reading the
      manifest rather than `cfg!`).

## 2. Record the attempted handover

- [ ] `docs/PHASE4-PANE-READINESS.md`: add §14 — the release blocker with its
      evidence chain, the three pane-level blockers §5 of the design lists, and the
      finding that the acceptance snapshots cannot witness this pane's replacement.
- [ ] `docs/PHASE6-TEARDOWN-READINESS.md`: §3 gains the third condition; §4's
      worklist ordering is corrected so Stage B precedes the seven pane handovers.
- [ ] `docs/ARCHITECTURE.md`: ADR-37, the precondition as a decision with its
      rejected alternatives.
- [ ] Verify: `rumdl check .`

## 3. Whole-tree verification before commit

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --features plugins -- -D warnings`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [ ] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [ ] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [ ] `./scripts/dev/lint-luau.sh`
- [ ] `git diff --stat` over `src/app/snapshots/` is empty — no snapshot moved,
      because no renderer changed.
- [ ] Drive the real thing: `scripts/dev/sandbox.sh --fresh --plugins --show info`
      and confirm the plugin pane and the native info panel are both on screen and
      agree, which is the state this change leaves in place deliberately.
