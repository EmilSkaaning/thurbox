# Stage B: the plugin host ships in the build a user installs

## Why

ADR-37 stopped the first pane handover with a finding that is not about any pane:
**nothing a plugin draws reaches a released binary.** `Cargo.toml` reads
`default = []`, the plugin runtime is `plugins = ["dep:mlua"]`, a required CI step
asserts the default dependency tree contains no `mlua`, and
`release/workflow-invariants` *specifies* that the release workflow never builds
with the feature. So a bundled pane is a Luau program, a Luau pane needs the VM,
and the VM is provably absent from every install. Handing a pane over today would
delete that pane from every installed copy of thurbox while
`cargo nextest run --all --features plugins` stayed green — the failure absent
from the build that ships and invisible in the build that is tested hardest.

ADR-37 recorded flipping the default as the **rejected** alternative, and named
what it would cost: an MSRV rise, vendored C++ sources in the path of four
release targets, and the contradiction of a specified release invariant plus a
required CI assertion. It rejected it *for that change* — a pane port is not
where a release decision belongs — and left it as step 7 of
`docs/PHASE6-TEARDOWN-READINESS.md` §4's worklist, upstream of every pane
handover.

This is that step, priced. It is one line of `Cargo.toml` and four consequences
that each have to be handled rather than discovered:

1. **The MSRV rises 1.86 → 1.88.** `mlua` declares 1.88 and cargo cannot express
   a per-feature minimum; with `plugins` in `default`, the runtime's floor *is*
   the crate's floor. Four documents state a floor of 1.75, which has been false
   since `ratatui 0.30`.
2. **A required CI assertion inverts.** "Assert the default build excludes the
   plugin runtime" becomes an assertion that it includes it. The direction of the
   invariant changed; the need for one did not.
3. **A specified release invariant becomes obsolete, and worse than obsolete.**
   `release/workflow-invariants` requires that `cd.yml` never asks for the plugin
   feature. After the flip no release job asks for it *and every release binary
   contains it*, because it arrives through the default feature list — so the
   check would keep reporting `ok` about the exact thing it claims to forbid.
4. **The runtime is compiled from vendored C++**, so every release target needs a
   C++ compiler and, when cross-compiling, a C++ standard library for that
   target. A release that fails to build on one platform is the worst outcome
   available here, so the four targets are measured rather than assumed.

The v1 behaviour at stake is the shape of the installed binary. Before this
change, an installed thurbox has no plugin host: no discovery, no VM, no
`thurbox-cli plugin` verbs, no `F10`. After it, every install has all of them,
and that is the point of Stage B — the plugin API gets real users while it is
still cheap to fix, because [N4] makes it additive-only from 2.0.0 onward.
Nothing is taken away: all seven native panes stay, drawn by the same renderers,
and no bundled pane appears on screen unasked.

## What Changes

- **`plugins` joins the default feature set**, and `rust-version` rises to 1.88
  with `clippy.toml` following it. The per-feature-MSRV comment in `Cargo.toml`
  described a problem cargo has and the manifest no longer needs to work around;
  it is corrected rather than deleted.
- **The CI assertion inverts and stays required.** The `plugins` job's final step
  asserts the default dependency tree *contains* `mlua`. The job keeps its
  pinned 1.88 toolchain — which is now a real MSRV floor check rather than a
  workaround — and its Luau type-check.
- **The job gains the configuration nothing else covers any more.** With the
  runtime in `default`, every other Rust job compiles it; what becomes untested
  is the build *without* it. `--no-default-features` is the documented fallback
  for a platform where the vendored C++ will not build, so the job compiles and
  tests that configuration instead of duplicating the default one.
- **Release invariant 2 is replaced, not dropped.** The requirement that `cd.yml`
  never builds *with* the plugin feature is removed with its reason, and a
  requirement that it never builds *without* the runtime takes its place —
  rejecting `--no-default-features` and a manifest edit to the default feature
  list. The hazard reversed direction: once a pane is handed over, a release
  built with default features suppressed ships that pane as an empty column.
- **The bundled example pane stops being visible by default.** `hello`'s
  manifest omits `default_visible`, and the seed defaults to `true` — correct for
  a plugin an author installed on purpose, wrong for one that arrives inside the
  binary. Under `default = []` nobody could see it; under `default = ["plugins"]`
  every fresh launch would open a "Hello" pane in the right column. It is seeded
  hidden, and a test holds the rule for the whole bundled set rather than for the
  one manifest that remembered it.
- **The teardown gate's build condition flips honestly.** The probe is unchanged —
  it reads `Cargo.toml`'s default feature list, which is exactly the fact this
  change edits. What changes is the test that pinned the old answer: it asserted
  the runtime is *absent* from the default build, and now asserts it is present
  and that each pane row is blocked by its own pane-level reason. The pure rule
  ("a pane drawn only by a gated build is not handed over") stays, because it is
  what a future change removing the runtime from `default` would violate.
- **Four documents' MSRV statements are corrected**, and ADR-40 records the
  decision with the four-target measurement, so the next reader meets the price
  in `docs/ARCHITECTURE.md` rather than paying it at release time.

## Capabilities

- `release/workflow-invariants` — invariant 2 is removed with its reason and
  replaced by its inverse.
- `plugin-host/runtime` — the runtime's membership of the default build, the MSRV
  that follows from it, and the per-target C++ toolchain the vendored sources
  need, all become stated requirements rather than facts of a workflow file.
- `plugin-host/pane-visibility` — no bundled pane is on screen before a user asks
  for it, checked over the whole bundled set.
- `migration/teardown` — the handover condition about the build stays checked
  after it is satisfied, so a later change that removes the runtime from
  `default` fails the inventory instead of quietly emptying every handed-over
  pane.

## Non-goals

- **Handing over any pane.** No native renderer is deleted and `src/app/view.rs`
  is untouched. This change unblocks the seven handovers; it does not perform
  one. Six of the seven still need a view-write channel
  (`docs/PHASE6-TEARDOWN-READINESS.md` §4 step 8) and a pane slot for the region
  they occupy.
- **The runtime `[features]` flag and Stage C.** Stage B is the *Cargo* default.
  There is no runtime `[features] plugins` setting today and this change does not
  add one: the host is additive — discovery over an empty plugin directory, no
  visible pane — so a switch would gate nothing a user can see. Bundling, the
  `2.0.0` cut, and the replacement of `[features]` flags by
  `plugin enable|disable` stay Stage C.
- **Cutting the release.** The version that carries a stage transition is chosen
  by hand, via `cd.yml`'s `workflow_dispatch` `version` input, precisely because
  the train would compute one from a commit type. Nothing here tags anything.
- **Widening any host surface.** No capability, node kind, style token, pane slot
  or binding. The only intended `src/` edit is one manifest line; see Impact for
  the one lint the MSRV bump forced on top of it.
- **Removing the `plugins` feature.** It stays as the switch that produces a
  binary without the vendored runtime, and is now CI-tested in that direction.

## Impact

- Manifest: `Cargo.toml` (default features, `rust-version`), `clippy.toml`.
- CI: `.github/workflows/ci.yml` (the `plugins` job).
- Checks: `scripts/dev/lint-workflows.sh` + `scripts/dev/lint-workflows.bats`
  (invariant 2), `tests/teardown_gate.rs` (one test and its module notes),
  `tests/bundled_manifests.rs` (new).
- Plugin: `src/plugin/bundled/hello/plugin.toml` — one line.
- `src/app/mod.rs` + `src/app/automation.rs`: eight `% N == 0` tick-cadence
  checks rewritten as `is_multiple_of`. Not planned — raising `msrv` to 1.88
  un-suppresses `clippy::manual_is_multiple_of` (stabilised in 1.87), which fails
  under `-D warnings`. It is the only reason this change touches `src/` logic at
  all, and the rewrite is behaviour-preserving.
- Docs: `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `openspec/config.yaml`
  (MSRV), `docs/ARCHITECTURE.md` (ADR-37's rejected alternative, new ADR-40),
  `docs/PHASE4-PANE-READINESS.md` §14 and `docs/PHASE6-TEARDOWN-READINESS.md`
  §3–§4 (the release blocker is cleared), `scripts/dev/sandbox.sh` (`--plugins`
  is no longer what makes the host present).
- No `src/` behaviour change: every `cfg(feature = "plugins")` site is unchanged,
  and the code they gate is the code that now compiles by default.

[N4]: the plugin API is additive within a major version.
