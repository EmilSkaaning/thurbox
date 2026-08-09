# Thurbox v2 — Migration Plan

How v1 becomes v2 without a big-bang rewrite, what gets deleted, and what breaks
for users.

The delivery mechanics — one trunk, a Cargo feature gate, three stages, and the
nightly channel — are specified in
[RELEASE-STRATEGY.md](RELEASE-STRATEGY.md). This document is the work
breakdown that runs inside them.

---

## 1. Principles for the transition

1. **`main` stays shippable, and stays v1.** Every phase lands on `main` behind
   the `plugins` Cargo feature, which is absent from stable builds until Stage B
   ([RELEASE-STRATEGY §2](RELEASE-STRATEGY.md#2-the-delivery-model)).
   v1 releases continue on their existing cadence, from the same trigger, with
   the same process.
2. **Additive before subtractive.** Everything that *adds* a capability ships
   before anything that *removes* one. The plugin host reaches real users at
   Stage B, in a v1 minor release, with every native pane intact. Only Stage C
   deletes.
3. **The acceptance harness is the safety net.** `src/app/acceptance.rs` (the
   in-process `Harness`, the insta snapshots, the invariant monkey test, the
   `perf_*` counter tests) already pins TUI behavior. A pane is migrated when
   its plugin implementation passes the *same* tests the Rust pane passed —
   asserted against both implementations in the same run, since both are
   present.
4. **Answer the hardest question early; land the hardest pane late.** The
   session list is the pane that decides whether
   [ADR-V1](ARCHITECTURE.md#adr-v1) holds. Prototype it as a throwaway spike in
   Phase 1, against the pass/fail bar in §3, *before* six other panes are built
   on the assumption it works; land it for real in Phase 4. Spiking early and
   shipping last are not in tension: the spike exists to answer a question, not
   to produce code.
5. **Delete when the plugin becomes the default, not when it lands.** This
   amends the earlier "no dual implementations" rule, and the reasoning is in
   [RELEASE-STRATEGY §5](RELEASE-STRATEGY.md#5-why-coexistence-beats-divergence).
   Deletion is still per-pane and still one PR; it just happens at Stage C
   rather than at merge time.

---

## 2. Phases

| Phase | Work | Gate | Stage |
|---|---|---|---|
| **0** | Foundations — release plumbing, kernel boundary, Luau toolchain | ungated | A |
| **1** | Plugin host — VM lifecycle, host bindings, `@thurbox` modules | `plugins` | A |
| **2** | Service half — headless plugins, storage, CLI verbs, bus | `plugins` | A |
| **3** | Motion and real-time surfaces | `plugins` | A |
| **4** | Bundled plugins, additive alongside the native panes | `plugins` | A |
| **5** | Command registry and agent API | `plugins` | A |
| — | **Stage B ships**: feature default-on, runtime opt-in, v1 minor release | default-on | B |
| **6** | Teardown, defaults flipped, 2.0.0 | default | C |

Phases 2 and 3 are independent of 4 and can run in parallel with it; they share
only the supervisor. Everything else is ordered.

### Phase 0 — Foundations (no behavior change)

Establishes the seam and the tooling, with nothing gated and nothing plugin-shaped
yet. All of it is either behavior-preserving or pure CI, so it ships to v1 users
immediately and gets exercised by them before anything depends on it.

**Release plumbing** ([RELEASE-STRATEGY §6](RELEASE-STRATEGY.md#6-ci),
[§7](RELEASE-STRATEGY.md#7-the-installer-hazard)):

- Add the `--features plugins` CI matrix leg (build, clippy, nextest), required
  rather than advisory.
- Add the Luau toolchain — `luau-analyze` in strict mode plus the plugin test
  runner — as CI jobs and pre-commit stages, **before** the first Luau PR, so
  that PR does not also carry the toolchain.
- Harden both installer version fallbacks to accept only
  `v<major>.<minor>.<patch>`, with a prerelease fixture regression test in
  `install.bats` and `install.Tests.ps1`.
- Add the workflow-invariant lint for
  [RELEASE-STRATEGY §9](RELEASE-STRATEGY.md#9-invariants) 1–4.
- Verify cocogitto's behavior on non-semver tags
  ([RELEASE-STRATEGY §4.2](RELEASE-STRATEGY.md#42-tag-naming-and-why-it-is-not-semver)).

**Kernel boundary:**

- Identify which of `App`'s 80 fields survive as kernel state and which are pane
  state. `src/app/mod.rs` is 14,605 lines; the output of this step is a map, not
  a refactor.
- Introduce the `plugin` module in `tests/architecture_rules.rs`
  ([ADR-V12](ARCHITECTURE.md#adr-v12)) with no contents yet.
- Introduce the view-tree data types in `session::view`, so `ui` can render them
  without importing `plugin` — mirroring why `session::review` exists for diffs.
- Add a **native view-tree renderer** in `ui/` over ratatui
  ([ADR-V13](ARCHITECTURE.md#adr-v13)) and reimplement one simple v1 pane (the
  info panel) through it, still in Rust, still in-process.
- Replace `compute_layout`'s 10 fixed rects with the **workspace tree**
  ([ADR-V23](ARCHITECTURE.md#adr-v23)), shipping only the synthesized default
  preset — no `layout.toml`, no new capability. The insta snapshots must not
  move, which is exactly the test that the preset reproduces v1.
- Add the **overlay layer and `anchor` resolution**
  ([ADR-V22](ARCHITECTURE.md#adr-v22)), and port v1's `render_compose_inline`
  onto it. This retires the `diff.inlineAt` special case before anything
  depends on it.

Both layout items belong here rather than later: they are behavior-preserving
refactors of kernel rendering, and doing them after panes are plugins would mean
migrating every pane twice.

This phase is smaller than it sounds: v1 already hands 19 pure view-model structs
(`TaskPaneState`, `LeftPanelState`, …) to pure renderers that emit `Line`/`Span`.
Phase 0 collapses 19 typed view models into one node set — it does not introduce
the pattern.

**Exit criteria**: the info panel renders through the view tree with
byte-identical insta snapshots; the `--features plugins` leg is green against an
empty feature; both installers resolve to stable with a prerelease fixture
present.

### Phase 1 — Plugin host

- Embed `mlua` with the `luau` backend ([ADR-V2](ARCHITECTURE.md#adr-v2)).
- VM supervisor: create, sandbox, per-VM memory limit, interrupt handler,
  thread per plugin, backoff, suspend/resume
  ([ADR-V4](ARCHITECTURE.md#adr-v4)). The sandbox binds **only** the host
  functions a plugin's capabilities grant — no `io`, no `os.execute`, no
  `os.getenv`, no `require` outside the plugin's own directory
  ([SECURITY.md §3](SECURITY.md#3-resolved--fs-net-and-shell-are-now-enforced)).
- The install prompt names **remote** reach for `sessions = "control"`
  ([SECURITY.md §9](SECURITY.md#9-remote-blast-radius)).
- The `@thurbox` module: `definePlugin`, `defineService`, `ui.*` node
  constructors, and the type definitions `luau-analyze` checks against.
- `@thurbox/widgets`: `list`, `table`, `badge`, `keyHints`, `empty`
  ([ADR-V14](ARCHITECTURE.md#adr-v14)).
- `thurbox plugin` CLI: `list`, `info`, `dev`, `reload`, `doctor`.
- **Keybindings and pane visibility**
  ([FEATURES-Keybindings.md](FEATURES-Keybindings.md),
  [ADR-V21](ARCHITECTURE.md#adr-v21)) — needed here rather than in Phase 5,
  because a pane is not usable without a way to show it. Open `pane:<id>`
  contexts; manifest defaults that are dropped rather than stolen on
  collision; automatic passthrough deferral for global bare-`Ctrl+<letter>`
  plugin chords; kernel-owned per-pane visibility with generated
  `<plugin>.<pane>.{toggle,show,hide}` commands, persisted, waking a suspended
  plugin on show; plugin sections in the F1 editor.
- **The session-list frame-budget spike** (§3). Throwaway code, answering one
  question.

**Exit criteria**: a hello-world plugin renders a `right`-slot pane, receives key
events, and hot-reloads on save; its pane toggles from a rebindable chord and
its keys appear in F1; the spike has produced a number against §3's bar and a
recorded decision.

### Phase 2 — Service half

The backend contract ([FEATURES-Backend-API.md](FEATURES-Backend-API.md)).

- Service hosting across all three hosts (TUI, heartbeat keeper, CLI invocation)
  with the machine-wide advisory lock ([ADR-V16](ARCHITECTURE.md#adr-v16)).
- `ctx.kv` plus kernel-executed namespaced migrations
  ([ADR-V17](ARCHITECTURE.md#adr-v17)), with `plugin_migrations` and namespace
  rejection tests.
- CLI verb registration and collision checks against kernel subcommands.
- Supervised `services`, `schedules` routed through `claim_due_automation`, the
  event bus, and cross-half typed RPC.
- Spawn contributions (append-only, fail-open) with the **env-key denylist** —
  `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, `GIT_SSH_COMMAND`, `GIT_EXTERNAL_DIFF`,
  `BASH_ENV`, `NODE_OPTIONS`, and `PATH` prepends confined to `{plugin_data}`
  ([SECURITY.md §8](SECURITY.md#8-spawn-contributions)) — and the `skills`
  registry. The denylist ships with the feature, not after it: append-only
  bounds who wins, not what can be injected.

**Exit criteria**: a service-only plugin syncs data with the TUI closed, driven by
the heartbeat keeper, and owns a working `thurbox-cli <verb>`.

### Phase 3 — Motion and real-time surfaces

Independent of Phase 4 — it touches the renderer and the session layer, not any
bundled pane — and it is what makes the plugin API demonstrable to third parties
rather than only to ourselves.

- `motion` evaluation on the frame clock, per-pane animation leases, the per-pane
  and aggregate rate caps, and a `reduce_motion` setting
  ([ADR-V18](ARCHITECTURE.md#adr-v18), specified in
  [FEATURES-Animation.md](FEATURES-Animation.md)). `statusDot` re-expressed as a
  kernel-supplied `cycle` is the first consumer. Two rules carry most of the risk
  and need tests first: motion state keyed so an identical re-push does **not**
  restart an animation ([§3](FEATURES-Animation.md#3-identity-and-phase)), and
  evaluation through the existing `app::clock` test clock so insta snapshots stay
  pinned at frame 0 ([§9](FEATURES-Animation.md#9-determinism-and-testing)).
- `pty` and `surface` nodes over the existing `vt100` + `tui-term` path:
  spawn/supervise, grid lifetime keyed to the node id across suspend and reload,
  `SIGWINCH` from the resolved rect, `surface/write` with backpressure
  ([ADR-V19](ARCHITECTURE.md#adr-v19)).
- Input sinking for a focused grid node, the `escape` chord, and
  `keyReport: "press-release"` — pushing kitty `REPORT_EVENT_TYPES` on focus and
  popping it on blur. This is the one piece with a blast radius outside the plugin
  system, so it ships with a test asserting an unfocused grid leaves every agent
  pane's input encoding byte-identical to v1.

**Exit criteria**: `perf_*` counters show an idle TUI unchanged with an animated
pane present but hidden; a plugin embeds a full-screen interactive program
(held-key input working) and survives tabbing away and a hot reload.

### Phase 4 — Bundled plugins, easy first

Each pane is built as a plugin and lands **alongside** its native implementation,
selected by the runtime flag. Migrate in ascending order of coupling, one PR
each:

| Order | Pane | Why this position |
|---|---|---|
| 1 | Info panel | Read-only, no input, already view-tree in Phase 0 |
| 2 | Tasks | Self-contained state, kernel-table API exercises `host/*` |
| 3 | Automations | Adds an in-pane editor and run history — tests `input`/`table` |
| 4 | File viewer | Tests the `fileTree` kernel surface and find sub-mode |
| 5 | Global search | Tests the `bottom` slot and cross-pane query broadcast |
| 6 | Code review | Largest: tests `diff`, `anchor` compose boxes, and the changed-files pane |
| 7 | Session list | Tests nesting, ordering, status dots, and `sessionTerminal` |

The session list is last by coupling, but its frame-budget spike ran in Phase 1
(principle 4). If that spike said no, the fallback is a kernel `sessionList`
surface ([ADR-V6](ARCHITECTURE.md#adr-v6)) that a plugin configures — a real
retreat from [ADR-V1](ARCHITECTURE.md#adr-v1), and one that must be discovered
before the other six panes are built on the opposite assumption.

**Installation lands here too**, because Stage B's exit criterion requires a
plugin thurbox did not write, and that plugin has to arrive somehow:
`thurbox plugin install|uninstall|enable|disable`, resolving a registry name, a
git URL, or a path; the capability prompt; and the supply-chain controls that
must exist before the first third-party install — a **lockfile recording
resolved version and content hash** per plugin and dependency, deterministic
re-resolution with no floating ranges, and official plugins pinned to the
binary's release tag as v1 pinned extensions
([SECURITY.md §6](SECURITY.md#6-supply-chain)).

One kernel gap is scheduled here rather than hand-waved
([FEATURES-View-Tree.md §11](FEATURES-View-Tree.md#11-expressiveness-check)):
a kernel-broadcast search query for cross-pane match highlighting (order 5).
The other gap that audit found — a compose box anchored to a diff line — was
closed generally by `anchor` in Phase 0
([ADR-V22](ARCHITECTURE.md#adr-v22)) rather than by a `diff`-only slot.

**Exit criteria**: with `plugins = true`, every pane above renders as a plugin and
reproduces its native predecessor's insta snapshot; with `plugins = false`, v1 is
byte-identical.

### Phase 5 — Command registry and agent API

- Registry generalized from the keybinding lookup built in Phase 1 into the
  full command registry, with the palette on top of it.
- `thurbox-cli command list|describe|run`.
- Control socket hardening: mode `0600` in a `0700` directory, peer-UID check,
  an explicit Windows DACL, unlinked on shutdown and on the panic path
  ([SECURITY.md §5](SECURITY.md#5-the-control-socket)). It exposes `command/run`
  to any local process otherwise.
- Conservative `agent_policy` defaults — destructive verbs default to `confirm`
  or `deny`, and capability-widening commands are permanently `deny`
  ([SECURITY.md §7](SECURITY.md#7-agent-driven-execution-and-prompt-injection)).
- Control socket, caller policy, loop guard
  ([FEATURES-Agent-API.md §5](FEATURES-Agent-API.md#5-permission-model)).

**Exit criteria**: every bundled plugin command is listable and invocable from
inside a session.

### Stage B — experimental release

Not a phase of work but a decision point
([RELEASE-STRATEGY §3](RELEASE-STRATEGY.md#3-three-stages-three-gate-positions)):
the Cargo feature flips to default-on and ships in an ordinary v1 minor release,
runtime-gated off. Native panes remain the
default. Plugin authors get a stable binary to build against, and the protocol
gets its exposure before N4 freezes it.

**One decision must be closed before this flip, not after it**: whether `fs`,
`net`, and `shell` are enforced or advisory
([SECURITY.md §3](SECURITY.md#3-resolved--fs-net-and-shell-are-now-enforced)).
Stage B is the moment third-party code starts running on users' machines under a
stated trust model, and the model has to be true when it is stated. Choosing
enforcement means a runtime reversal ([ADR-V3](ARCHITECTURE.md#adr-v3)), which
is far cheaper here than after plugins exist; choosing advisory means amending
[N3](CONSTITUTION-DELTA.md#n3--capabilities-are-declared-gated-and-shown) and
saying so at the install prompt.

**Exit criteria for leaving Stage B**: one full minor release with no
non-additive protocol change, and at least one plugin that thurbox did not write.

### Phase 6 — Teardown and 2.0.0

- Flip the runtime default to `true`
  ([ADR-V3](ARCHITECTURE.md#adr-v3)) and absorb the artifact-size increase across
  all four platforms and every packaging channel.
- Per pane, one PR: make the plugin the default and delete the native
  implementation with its `InputFocus`, `Action`, `PanelAreas`, `ClickAction`,
  and `FeatureFlags` entries.
- Delete the v1 extension system (§4).
- Absorb hooks into the kernel session layer.
- Replace `[features]` flags with `thurbox plugin enable|disable` — the
  *existence* axis. The **visibility** axis (`F2`/`F3`/`F5`/`F9`) is a
  separate mechanism that must already be in place: kernel-owned per-pane
  visibility with auto-generated `<plugin>.<pane>.toggle` commands
  ([ADR-V21](ARCHITECTURE.md#adr-v21),
  [FEATURES-Keybindings.md §7](FEATURES-Keybindings.md#7-pane-visibility)).
  Collapsing the two loses a v1 behavior.
- Documentation: fold `docs/v2/` into `docs/`, rewrite `CLAUDE.md`, move the
  website's `/v2/` tree to `/`.
- Ship `2.0.0` via the explicit-version release dispatch (`cog bump --version
  2.0.0`), since `--auto` will not cross a major boundary on its own.

---

## 3. The session-list decision gate

Open question 1 (§7) is the one that can invalidate
[ADR-V1](ARCHITECTURE.md#adr-v1), so it gets a bar defined **in advance** rather
than a judgement call made under sunk cost. The Phase 1 spike passes only if all
four hold, measured through the existing `perf_*` counters and
`THURBOX_PERF_LOG=1`:

| Measure | Bar | Why this number |
|---|---|---|
| `view/push` rate, 20 sessions at 5 status transitions/s | ≤ 10 Hz sustained | The per-pane push ceiling in [LIMITATIONS §6](LIMITATIONS.md#6-cost-ceilings). Above it, the pane is fighting the protocol |
| `first_frame_ms` with the default bundled set active | ≤ 115% of v1 | v1's number is the baseline users have; 15% is the largest regression that does not read as "slower to start" |
| Idle paint rate, no activity | Unchanged from v1's ~4 fps floor | The demand-driven loop (ADR-P1–P12) is the asset [ADR-V11](ARCHITECTURE.md#adr-v11) exists to protect |
| Added input→paint latency on selection change | ≤ 5 ms | The ceiling already claimed in [LIMITATIONS §6](LIMITATIONS.md#6-cost-ceilings) |

Note the second row: measuring `first_frame_ms` with *no plugins active* is
trivially satisfied by lazy activation and proves nothing. The bar is the default
bundled set.

**If the bar is missed**, the retreat is the kernel `sessionList` surface, taken
at the end of Phase 1 and recorded as an amendment to ADR-V1 — not discovered in
Phase 4 with six panes already built on the assumption.

---

## 4. Teardown inventory — the v1 extension system

Deleted outright ([ADR-V8](ARCHITECTURE.md#adr-v8)):

| Path | Lines | Notes |
|---|---|---|
| `src/session_ops/extensions.rs` | 2,368 | install/uninstall/reinstall/update/heal |
| `src/agent/extension_config.rs` | 1,070 | manifest loading, agent patches, config merges |
| `src/session/extension_def.rs` | 794 | `ExtensionDef`, version helpers |
| `src/cli/extensions.rs` | 613 | the `thurbox-cli extension` subcommand + dispatch |
| `src/agent/json_merge.rs` | 183 | only consumer was `[[config_merges]]` |
| **Total deleted** | **5,028** | |
| `extensions/` | 91 files, 580 KB | flow, forge, ci-shepherd, renovate, 4 tracker integrations, hooks |
| `metadata.active_extensions`, `metadata.builtin_hooks_optout` | — | schema cleanup migration |

Rewritten rather than deleted:

| Path | Lines | Fate |
|---|---|---|
| `src/session_ops/builtin_hooks.rs` | 554 | Absorbed into the kernel session layer — the behavior is core product, not an extension |
| `src/session_ops/remote_hooks.rs` | 646 | Untouched. Session infrastructure, not extension infrastructure |

So: **5,028 lines of Rust deleted, 554 absorbed, and the `extensions/` tree
removed.**

### What must not be lost

| v1 capability | v2 home |
|---|---|
| Agent status hooks (working/blocked/done) | **Kernel.** Core product behavior — moves into the session layer, including remote hook rewriting and the psmux gate |
| Registering agents in `agents.toml` | Plugin manifest `[[agents]]` contribution |
| Seeding sessions/automations | Plugin manifest `[[automations]]` + `init` via `host/*` APIs |
| Placing files in agent config dirs | Plugin `fs` capability |
| Patching agent args at spawn | Spawn contributions ([FEATURES-Backend-API.md §11](FEATURES-Backend-API.md#11-spawn-contributions)) |
| Self-heal on startup/tick | Plugin activation is idempotent by construction |
| Version/staleness/auto-update | `thurbox plugin update`, same release-tag pinning |

---

## 5. User-facing breakage

Nothing breaks before 2.0.0. Stage B adds a capability and removes none.

v2.0.0 is a **breaking major release**. What changes:

| v1 | v2 | Migration |
|---|---|---|
| `[features] tasks = false` | `thurbox plugin disable tasks` | Automatic: the settings loader translates known flags on first run and writes a deprecation note |
| `thurbox-cli extension …` | `thurbox plugin …` | Command removed; error message points at the replacement |
| `extensions/` installs | Gone | Not migrated — the extensions are unused ([ADR-V8](ARCHITECTURE.md#adr-v8)) |
| ~10 MB binary | ~60–100 MB archive | Documented in release notes and on the install page |
| `settings.toml`, `agents.toml`, `hosts.toml`, `themes.toml`, `keybindings.json` | Unchanged | None |
| SQLite database | Forward migration from schema 40 | Automatic on first open, as with every prior schema bump |
| tmux sessions, worktrees, branches | Unchanged | v2 adopts running v1 sessions normally |

**The valuable state — live sessions, worktrees, tasks, automations, message
history — carries over untouched.** Only the extensibility surface breaks.

---

## 6. Test strategy

The existing harness carries over and is extended:

| Layer | v1 | v2 addition |
|---|---|---|
| In-process acceptance (`Harness`) | Feeds `AppMessage`, renders to `TestBackend` | Plugins stubbed by a fake returning canned view trees — no VM in the normal test suite |
| insta snapshots | 7 pinned screens | Same screens through the view-tree renderer. During Phase 4 each screen is asserted against **both** implementations in one run |
| Invariant monkey test | Random events, `assert_invariants` after each | New invariants: focus never lands on a suspended plugin's pane; a faulted plugin never blocks a frame; no pane renders a tree it did not push; motion invariants ([FEATURES-Animation.md §9](FEATURES-Animation.md#9-determinism-and-testing)) |
| Perf counters (`perf_*`) | Idle iterations skip the paint, order cache rebuilds | New counters: pushes per second, dropped frames, RPC deadline misses, motion leases |
| Protocol conformance | — | New: a fixture plugin exercising every node type and every failure mode (slow render, crash on init, oversized tree, malformed frame) |
| Plugin unit tests | — | New: a Luau harness in `@thurbox` running reducers against event sequences, no kernel needed |
| End-to-end | `scripts/dev/smoke/tui-smoke.sh` | Extended to boot with a real runtime and real bundled plugins, in the `--features plugins` configuration |
| Feature-gate integrity | — | New: CI builds and tests both configurations; the ungated build must not link the runtime supervisor |

---

## 7. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Session list cannot meet the frame budget as a plugin | Medium | High | The §3 bar, measured in Phase 1 before six panes depend on the answer; kernel `sessionList` surface as the recorded retreat |
| First-frame regression from N cold starts | Medium | High | Activation events ([ADR-V15](ARCHITECTURE.md#adr-v15)) bound N to plugins in use; render kernel surfaces before plugins attach; §3's `first_frame_ms` bar |
| Gated code rots — `--features plugins` breaks unnoticed | Medium | Medium | The matrix leg is a required check, not advisory ([RELEASE-STRATEGY §9](RELEASE-STRATEGY.md#9-invariants) invariant 7) |
| Native and plugin implementations drift during Phase 4 | Medium | Medium | Both assert the same insta snapshot in the same run; the window is bounded and ends at Phase 6 |
| Kernel node catalog becomes the bottleneck for plugin authors | Low | High | Two-tier catalog ([ADR-V14](ARCHITECTURE.md#adr-v14)) — widgets are userland Luau, so one kernel gap remains across all 9 v1 surfaces |
| Artifact size breaks a packaging channel | Medium | High | Deferred to Stage C by [RELEASE-STRATEGY §3](RELEASE-STRATEGY.md#3-three-stages-three-gate-positions); validate Chocolatey/winget/AUR limits during Stage B, with months of warning |
| View tree cannot express the code review | Medium | High | Built in Phase 4 *before* the native one is deleted in Phase 6; its one hard case, the anchored compose box, is solved generally in Phase 0 |
| Two-language contributor friction | High | Medium | `@thurbox` type definitions, `plugin dev`, and a template repo; Luau is Lua-5.1-shaped, so neovim-config experience transfers |
| Luau proves awkward for a real pane | Medium | High | Phase 1's validation gate writes the code review or session list in it, and has an agent write it too, before six panes assume it works |
| Plugin API churn after third-party adoption | Medium | Medium | Protocol major version; additive-only rules; Stage B is the window in which mistakes are still free to fix |
| Scope creep — v2 never lands | High | High | Stage B is a shippable outcome on its own. Even if Phase 6 never happens, the plugin host is in users' hands and v1 is undamaged |

The last row is the structural difference from the branch model: under trunk-based
delivery, **abandoning v2 costs a feature-flag deletion**, and abandoning it
*after Stage B* still leaves users with a working plugin system on a stable v1.

---

## 8. Rollback

| Point of failure | Cost of stopping |
|---|---|
| During Phase 0 | Nothing to roll back. Every item is a CI or behavior-preserving improvement worth having on its own |
| During Phases 1–5 | Delete the `plugins` Cargo feature and its modules in one PR. Stable builds never contained them |
| At Stage B | Flip the Cargo feature back to default-off in a patch release. Users who opted in lose plugins; nothing else changes |
| During Phase 6 | The riskiest window, and the only one that needs care: pane deletions are irreversible within a release. Each deletion is its own PR and its own revert |
| After 2.0.0 | Normal major-version support: v1 stays installable at its last tag through every packaging channel |

---

## 9. Open questions

Deliberately unresolved, to be settled with evidence rather than in advance:

1. **Does the session list meet the frame budget as a plugin?** The bar is §3;
   the answer is due at the end of Phase 1.
2. **Does `mlua`'s vendored Luau build cleanly on every target?** Windows
   ARM64, musl, and the macOS universal build each need a green cross-build
   before [ADR-V3](ARCHITECTURE.md#adr-v3) is locked. This is a *build* question
   rather than a distribution one — the VM is linked in, so there is no separate
   runtime to ship or find — and it is answerable in CI during Phase 0 rather
   than by user reports. It replaces the earlier "which JS runtime, and does it
   exist on every target?", which the sidecar design could only settle with
   months of Stage B evidence.
3. **Do modals belong in the `overlay` slot or as a kernel primitive?**
   `ctx.ui.modal()` exists for kernel-driven prompts; whether plugins should own
   full modal panes is unsettled.
4. **How much of `git` needs a host API?** Phase 4's review migration will reveal
   whether `diff` as a kernel surface is enough or whether plugins need general
   git access.
5. **Suspension policy.** Sixty seconds is a guess. Real usage during Stage B
   should set it.
6. **Where exactly does the Cargo feature boundary fall in Phase 0?** The
   view-tree types and native renderer are behavior-preserving and arguably
   belong ungated in stable; the host is clearly gated. See
   [RELEASE-STRATEGY §11](RELEASE-STRATEGY.md#11-open-questions).
