# Design — the Phase 6 teardown inventory, and why it is a check

## 1. The question this change answers

`docs/v2/MIGRATION.md` §4 lists what Phase 6 deletes. It does not say how anyone
establishes that the deletion is safe. This change answers that, and the answer
for today's tree is **no**.

The evidence is below, gathered by reading the tree rather than the plan. Every
number in §2 and §3 was measured on the commit this change lands on.

## 2. Evidence — the deletion targets exist and are exactly the claimed size

`wc -l` on the paths MIGRATION §4 names:

| Path | MIGRATION §4 | Measured |
|---|---|---|
| `src/session_ops/extensions.rs` | 2,368 | 2,368 |
| `src/agent/extension_config.rs` | 1,070 | 1,070 |
| `src/session/extension_def.rs` | 794 | 794 |
| `src/cli/extensions.rs` | 613 | 613 |
| `src/agent/json_merge.rs` | 183 | 183 |
| **total** | **5,028** | **5,028** |
| `src/session_ops/builtin_hooks.rs` (absorb) | 554 | 554 |
| `src/session_ops/remote_hooks.rs` (keep) | 646 | 646 |
| `extensions/` | 91 files, 580 KB | 91 files, 580 KB |

So the inventory is accurate. Nothing about it is stale — which is precisely why
the *readiness* half of it needs the same treatment.

## 3. Evidence — six of the seven replacements do not exist

MIGRATION §4 "What must not be lost" lists seven v1 capabilities and names a v2
home for each. Probing the tree for those homes:

| v1 capability | v2 home per MIGRATION | Present today? |
|---|---|---|
| Agent status hooks | Kernel session layer | **No.** `ensure_builtin_hooks_extension` calls `install_extension`; the wiring is delivered *by* the installer that Phase 6 deletes |
| Registering agents in `agents.toml` | Manifest `[[agents]]` | **No.** `PluginManifest` has `panes`, `commands`, `keybindings`, `capabilities`, `service`, `cli`, `spawn` — no `agents` |
| Seeding sessions/automations | Manifest `[[automations]]` + `init` | **No.** No `automations` field, and no kernel-table host API a plugin could seed one through |
| Placing files in agent config dirs | Plugin `fs` capability | **No.** `Capability` is `log`, `state-read`, `state-write`, `render`, `input`, `spawn`. There is no `fs`, by design so far — SECURITY §3 enforces filesystem denial *by the absence of a binding* |
| Patching agent args at spawn | Spawn contributions | **No.** `SpawnDecl` carries `env` only. The Phase 4 note already recorded that argument contributions have no manifest surface: `PATH` prepends need an argv-level change that does not exist |
| Self-heal on startup/tick | Idempotent by construction | **Yes.** Discovery is a filesystem walk every host process performs at start (`plugin::discovery::discover`), so a contribution is re-derived rather than installed-then-healed. Nothing can go missing between runs |
| Version/staleness/auto-update | `thurbox plugin update` | **No.** The `plugin` CLI is `list`, `status`, `doctor`, `reload` |

One ready, six blocked. And the blockers are not details: three of them (`fs`,
`[[agents]]`, argument contributions) are *capability* decisions with security
consequences that SECURITY.md has not settled — `fs` in particular is currently a
deliberate absence, not an omission, so "add `fs`" is a design change and not an
implementation task.

The hooks row deserves its own sentence, because it is the one MIGRATION flags in
bold. The 554 lines are not the behaviour; they are the *delivery* of the
behaviour through the extension installer. Absorbing hooks means reimplementing,
in the kernel, the parts of `extension_config.rs` and `extensions.rs` that
hooks uses: `[[agent_patches]]` (reversible `toml_edit` surgery on `agents.toml`),
`[[external_files]]` (a `requires_dir` probe, a managed-marker guard, and
compare-before-write), and `[[config_merges]]` (a reversible deep merge with
prune-by-marker, whose only consumer is `json_merge.rs` — itself on the deletion
list). That is not a move; it is a rewrite of the safety rules that keep thurbox
from clobbering a user's `~/.gemini/settings.json`. Doing it half-way is worse
than not starting: the failure mode is silent, and what fails is every agent's
status dot.

## 4. Evidence — Phase 4 never ran, so pane deletion has no floor

Phase 6's per-pane step is "make the plugin the default and delete the native
implementation". `src/plugin/bundled/` contains one directory, `hello`, whose
manifest declares `capabilities = ["render"]` and one pane. None of the seven
panes MIGRATION §2 schedules for Phase 4 has a plugin implementation:

| Pane | Native renderer | Bundled plugin |
|---|---|---|
| Info panel | `src/ui/info_panel.rs` | absent |
| Tasks | `src/ui/tasks_panel.rs` | absent |
| Automations | `src/ui/automations_panel.rs` | absent |
| File viewer | `src/ui/file_viewer.rs` | absent |
| Global search | `src/ui/global_search.rs` | absent |
| Code review | `src/ui/code_review.rs` | absent |
| Session list | `src/ui/project_list.rs` | absent |

And Stage B — the release that must precede Phase 6 — has not happened either:
`Cargo.toml` still reads `default = []`, so no plugin host has ever reached a
user, and the Stage B exit criterion ("at least one plugin that thurbox did not
write") cannot have been met.

## 5. Why the inventory becomes a test rather than a document

Three reasons, in order of weight.

**A verdict is a fact about a build, and facts expire.** Every "No" in §3 is a
grep away from becoming a "Yes", and nothing about a table in a markdown file
notices. A gate that re-derives each verdict from the source and fails when the
recorded one disagrees cannot silently become a rubber stamp: implementing `fs`
makes the build tell you to revisit the row that depends on it.

**The failure it guards is silent and expensive.** Deleting a native pane whose
plugin does not exist is loud — the build breaks. Deleting the hooks installer is
quiet: the binary compiles, sessions launch, and status reporting stops for every
agent. That asymmetry is the argument for a check rather than a review habit.

**The repo already has this shape.** `tests/architecture_rules.rs` is an
allowlist over `src/` that fails when a module appears without a declared place.
A teardown gate is the same idea pointed at a different invariant: a path may not
disappear until its replacement is recorded as existing. Reusing the established
pattern means no new concept for a reader to learn.

The gate is a *source-level* check, which decides several things at once: it needs
no plugin feature (so it runs in both Cargo configurations), it needs no VM, and
its probes read the same text a human auditor would read. It is not a unit test
of behaviour; it is the executable form of a decision record.

## 6. Shape of the gate

Two tables and three rules.

`REPLACEMENTS` — one row per MIGRATION §4 "must not be lost" capability, each
with the v2 home it is promised, the recorded verdict, and a probe. A probe is a
predicate over the source tree (`PluginManifest` declares `agents`, `Capability`
lists `fs`, the `plugin` CLI has an `Update` variant, …).

`UNITS` — one row per thing Phase 6 deletes, each listing the paths it comprises
and the replacement ids that must be ready first:

- the **v1 extension system**, as one unit, because ADR-V8 deletes it as one and
  none of its five files is independently useful. Its requirement is all seven
  rows: the installer is what delivers every one of them.
- each **native pane**, requiring its bundled plugin directory to exist.

The rules:

1. Every path in every unit still exists, unless the unit's requirements are all
   ready. The failure message names the unmet ids — the reason, not just the rule.
2. Every recorded verdict equals its probe. The failure message names the row and
   which direction it drifted.
3. Readiness is a pure function of the verdict table, so "is deletion permitted?"
   is testable in both directions: today's table says no and names six blockers;
   a synthetic all-ready table says yes.

Rule 1 is deliberately generous about the extension unit's requirements. A finer
mapping — `json_merge.rs` needs only the hooks and config-dir rows,
`cli/extensions.rs` only the update row — would be defensible and is also a
judgement call I would be making on behalf of whoever does the deletion. The
gate's job in a destructive phase is to be conservative and to be *unarguable*,
so it asks for the whole set and lets the deleting change argue for a narrower
one by editing the table with its reasons attached.

## 7. The hook payload drift hole

Found while gathering §3, and worth fixing in this change because it is the
concrete form of "the hooks behaviour must not be lost".

`remote_hooks::remote_asset_for` is a kernel-side table mapping each config-dir
agent to its destination path, guard directory, and payload constant.
`remote_assets_stay_in_sync_with_embedded_manifest` asserts that table against
`extensions/hooks/extension.toml` — on `kind`, on `path`, on `requires_dir`. It
never compares the **payload**, because the manifest names its payload by source
filename (`source = "codex-hooks.json"`) and the filename-to-constant mapping
lives in a local variable inside `materialize_source`, unreachable from a test.

So a manifest row repointed at a different source file passes every existing
test: the local install writes one payload into `~/.codex/hooks.json` and the
remote provisioner ships a different one to the same path on a host. The
divergence is invisible until someone notices a remote session's dot never moves.

The fix is small and mechanical: hoist the table into `EMBEDDED_ASSETS`, have
`materialize_source` iterate it (identical behaviour — same filenames, same
order, same skip-if-unchanged write), and extend the sync test to resolve each
manifest row's `source` through it and compare bytes with the remote table's
payload. This also gives the eventual absorption a single named constant to move,
instead of a literal inside a function.

## 8. Alternatives considered

**Delete the user extensions (`extensions/flow`, `forge`, `ci-shepherd`,
`renovate`, the four trackers) now, keep `hooks`.** ADR-V8 asserts they are
unused, and deleting them would take ~63 files off the tree without touching the
hooks path. Rejected: "unused" is an assertion about *users*, which I cannot
verify from inside the repo, and `extension install <name>` resolves official
extensions against the release tag — so the tree is a published interface, not
just source. It is also not the constraint: those files are data, and their
removal unblocks nothing. The 5,028 lines of Rust stay either way.

**Implement the kernel hooks provisioner now, keep the extension path as dead
code.** Rejected for a specific reason rather than size: two provisioners writing
the same files means the switch-over is where the risk lives, and the switch is
not verifiable without the coverage that lives in `extensions.rs`'s own tests.
The safe order is provisioner and switch in one change, with the extension path
removed in the same breath — which is a whole session's work and is Phase 6's
first real task, not a subset of it.

**Add `fs`, `[[agents]]`, `[[automations]]` to unblock the table.** Rejected as
out of scope for a change whose subject is whether deletion is safe. Each is a
capability with a security argument to make (`fs` especially — SECURITY §3's
enforcement story is currently "there is no binding"), and adding three of them
to make a table go green would be inverting the point of having the table.

**Skip the gate; write the analysis in `MIGRATION.md`.** Rejected per §5. The
analysis is in this change either way; the gate is what makes it survive contact
with a future session that reads the phase label first.

## 9. What this change does not do

Nothing is deleted. `Cargo.toml` keeps `default = []`. No native pane, no
extension file, no metadata key, and no CLI verb is removed or deprecated, and
the version stays `0.0.0-dev`. The only behavioural change in the binary is that
`materialize_source` reads its asset list from a named constant.
