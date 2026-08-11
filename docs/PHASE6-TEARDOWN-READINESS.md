# Phase 6 teardown readiness — what may not be deleted yet, and why

Phase 6 of the v2 migration is the destructive one: delete the v1 extension
system, delete each native pane behind its plugin replacement, absorb the
built-in hooks behaviour into the kernel, flip the defaults, ship 2.0.0.

This document is the audit of whether any of that is safe today. **It is not.**
Every claim below is traced to the code that makes it true, and each is enforced
by `tests/teardown_gate.rs` (ADR-23) so it cannot quietly go stale — if you
implement one of these replacements, that test will tell you to come back here.

Status: **1 of 14 replacements ready.** No unit is deletable.

## 1. The deletion targets are real and exactly the claimed size

`wc -l`, against the inventory in the v2 design set (`MIGRATION.md` §4):

| Path | Claimed | Measured |
|---|---|---|
| `src/session_ops/extensions.rs` | 2,368 | 2,368 |
| `src/agent/extension_config.rs` | 1,070 | 1,070 |
| `src/session/extension_def.rs` | 794 | 794 |
| `src/cli/extensions.rs` | 613 | 613 |
| `src/agent/json_merge.rs` | 183 | 183 |
| **total to delete** | **5,028** | **5,028** |
| `src/session_ops/builtin_hooks.rs` (absorb) | 554 | 554 |
| `src/session_ops/remote_hooks.rs` (keep) | 646 | 646 |
| `extensions/` | 91 files, 580 KB | 91 files, 580 KB |

So the plan's arithmetic is sound. The readiness half is where it breaks.

## 2. Six of the seven "must not be lost" capabilities have no v2 home

| v1 capability | Promised v2 home | Present? | Evidence |
|---|---|---|---|
| Agent status hooks | kernel session layer | **no** | `ensure_builtin_hooks_extension` calls `install_extension` — the wiring is delivered *by* the installer Phase 6 deletes |
| Registering agents in `agents.toml` | manifest `[[agents]]` | **no** | `PluginManifest` declares `panes`, `commands`, `keybindings`, `capabilities`, `service`, `cli`, `spawn` |
| Seeding sessions/automations | manifest `[[automations]]` + `init` | **no** | no `automations` field, and no kernel-table host API to seed one through |
| Placing files in an agent's config dir | plugin `fs` capability | **no** | `Capability` is `log`, `state-read`, `state-write`, `render`, `input`, `spawn`, `sessions`, `metrics`, `automations` |
| Patching agent args at spawn | spawn contributions | **no** | `SpawnDecl` carries `env` only; argument contributions have no manifest surface |
| Self-heal on startup/tick | idempotent by construction | **yes** | `plugin::discovery::discover` re-walks the manifests every start, so nothing is installed-then-healed |
| Version/staleness/auto-update | `thurbox-cli plugin update` | **no** | the `plugin` CLI is `list`, `status`, `doctor`, `reload` |

Three of these are capability *decisions*, not implementation tasks. `fs` in
particular is a deliberate absence: filesystem denial is currently enforced by
there being no binding to reach through, so adding one is a security argument to
make, not a field to add.

The hooks row is the one that matters most. The 554 lines are not the behaviour;
they are its *delivery* through the extension installer. Absorbing hooks means
reimplementing, in the kernel, the parts of `extension_config.rs` and
`extensions.rs` that hooks depends on:

- `[[agent_patches]]` — reversible `toml_edit` surgery on `agents.toml` (claude's
  `--settings`, aider's `--notifications-command`), including the `hook_schema`
  fan-out to rebranded agents.
- `[[external_files]]` — a `requires_dir` probe, a managed-marker guard so a
  user-owned file is never clobbered, and compare-before-write (opencode, vibe,
  copilot, pi, omp).
- `[[config_merges]]` — a reversible deep merge with prune-by-marker into a file
  the user co-owns (codex's `hooks.json`, antigravity's `~/.gemini/settings.json`).
  Its only implementation is `json_merge.rs`, itself on the deletion list.

Getting any of that wrong fails silently: the binary compiles, sessions launch,
and status reporting stops. That is why it is one change with the switch-over in
it, not a refactor done in pieces.

## 3. Phase 4 has started, but no pane has been handed over

`src/plugin/bundled/` contains `hello`, `info-panel`, `tasks`, `file-viewer` and
`code-review`.

| Pane | Native renderer | Bundled plugin | Drawn by |
|---|---|---|---|
| Info panel | `src/ui/info_panel.rs` | `info-panel` | the native pane |
| Tasks | `src/ui/tasks_panel.rs` | `tasks` | the native pane |
| Automations | `src/ui/automations_panel.rs` | absent | the native pane |
| File viewer | `src/ui/file_viewer.rs` | `file-viewer` | the native pane |
| Global search | `src/ui/global_search.rs` | none possible yet (PHASE4 §10) | the native pane |
| Code review | `src/ui/code_review.rs` | `code-review` (the diff stream only, PHASE4 §11) | the native pane |
| Session list | `src/ui/project_list.rs` | absent | the native pane |

`docs/PHASE4-PANE-READINESS.md` is the audit of what the plugin API could not
express for the *first* of those panes; all five of its gaps are now closed
(ADR-26, ADR-27, ADR-28), and §8, §9 and §11 record what the second, third and
fourth ports needed on top of them (ADR-29, ADR-30, ADR-31).

One row in the table above will not fill in by porting harder. §10 of the same
document records **global search as structurally unportable** — it is a mode, not
a pane: it owns the interface's input, restyles rows in three panes it does not
own, and writes their cursors and the focus, none of which a plugin pane may do.
`tests/global_search_pane_gap.rs` holds that verdict as probes, so whoever closes
one of its blockers is told to revisit it.

The code-review row is the first that is **partly** filled: the bundled plugin
reproduces the unified diff stream's lines and nothing else, and §11 of the same
document itemises the rest of the view — the paired layout, the headers, the
comments, the marks, the find sub-mode, the target picker, the footer and the
compose box — with the reason each is unported. That row therefore needs both a
handover *and* the remaining surface before `src/ui/code_review.rs` can go, which
is a longer list than any other pane's.

**A pane's row is ready only on handover, not on existence.** Four panes now show
why the distinction is load-bearing rather than pedantic: each plugin exists and
reproduces its pane (the first three exactly, the fourth in the part it declares),
while the native renderer is still what the interface draws. Deleting
`src/ui/info_panel.rs` today would remove the pane every
user is looking at. So `tests/teardown_gate.rs`'s pane probe is a conjunction —
the bundled plugin exists **and** `src/app/view.rs` no longer names the pane's
native renderer module — and `a_reproduced_pane_is_not_a_replaced_one` pins that
reasoning so the probe cannot be "simplified" back to a directory check.

Handing a pane over is therefore its own step, distinct from writing its plugin:
it means `App::view` drawing the plugin's pane in the native one's place, which
needs the plugin pane to be reachable from the keyboard (PHASE4 §5) and to render
on events rather than on a 1 s poll (PHASE4 §7, and the session-list spike's third
condition). Neither is done.

Stage B has not happened either — `Cargo.toml` reads `default = []`, so no user
has ever run the plugin host, and Stage B's exit criterion ("at least one plugin
that thurbox did not write") cannot have been met. Phase 6 is two milestones
downstream of where the tree is.

## 4. Worklist, in dependency order

1. **Absorb hooks into the kernel** — a kernel provisioner covering the three
   delivery mechanisms above, the switch of `ensure_builtin_hooks_extension` onto
   it, and the removal of the extension path, in one change. Unblocks
   `hooks-in-kernel`, and with it most of the extension-system unit.
2. **Decide `fs`** — a capability with a security argument, or a narrower
   kernel-mediated "write into an agent's config dir" host call. Unblocks
   `agent-config-files`.
3. **Manifest `[[agents]]` and `[[automations]]`** — static contribution data in
   the shape `[spawn.env]` already established, resolved at the one funnel each
   already has. Unblocks `agent-registration`, `resource-seeding`.
4. **Argument spawn contributions** — the argv-level seam `PATH` prepends also
   need. Unblocks `spawn-arg-contribution`.
5. **`plugin install|update|enable|disable`** with the lockfile and release-tag
   pinning. Unblocks `plugin-update`, and is the prerequisite for replacing the
   `[features]` flags.
6. **Phase 4, all seven panes** — each landing alongside its native predecessor
   and asserting the same rendering. The info panel has landed this way
   (ADR-27, tree equality rather than a frame snapshot). Writing a pane's plugin
   does **not** unblock its unit; the seven pane units need the *handover* on top,
   which is per-pane keyboard visibility plus event-driven render, then
   `App::view` drawing the plugin in the native pane's place.
7. **Stage B**, then the flips: Cargo default, runtime default, `2.0.0`.

Nothing on this list is unblocked by deleting something first, which is the
whole finding: the teardown has no safe first step yet.
