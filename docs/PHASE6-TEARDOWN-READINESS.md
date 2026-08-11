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
| Placing files in an agent's config dir | plugin `fs` capability | **no** | `Capability` names no `fs`: it is `log`, `state-read`, `state-write`, `render`, `input`, `spawn`, `sessions`, `metrics`, `automations`, `tasks`, `files`, `tasks-write`, `automations-write`, `review` — every one of which reads or writes a record the kernel already holds |
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

`src/plugin/bundled/` contains `hello`, `info-panel`, `tasks`, `file-viewer`,
`code-review`, `session-list` and `automations`. Every native pane a plugin can be
written for now has one.

| Pane | Native renderer | Bundled plugin | Drawn by |
|---|---|---|---|
| Info panel | `src/ui/info_panel.rs` | `info-panel` | the native pane |
| Tasks | `src/ui/tasks_panel.rs` | `tasks` | the native pane |
| Automations | `src/ui/automations_panel.rs` | `automations` (its rows **and** five of its seven keys, PHASE4 §17) | the native pane |
| File viewer | `src/ui/file_viewer.rs` | `file-viewer` | the native pane |
| Global search | `src/ui/global_search.rs` | none possible yet (PHASE4 §10) | the native pane |
| Code review | `src/ui/code_review.rs` | `code-review` (the diff stream only, PHASE4 §11) | the native pane |
| Session list | `src/ui/project_list.rs` | `session-list` (its rows, PHASE4 §13) | the native pane |

`docs/PHASE4-PANE-READINESS.md` is the audit of what the plugin API could not
express for the *first* of those panes; all five of its gaps are now closed
(ADR-26, ADR-27, ADR-28), and §8, §9, §11, §13 and §17 record what the second
through sixth ports needed on top of them (ADR-29, ADR-30, ADR-31, ADR-33,
ADR-41).

One row in the table above will not fill in by porting harder. §10 of the same
document records **global search as structurally unportable** — it is a mode, not
a pane: it owns the interface's input, restyles rows in three panes it does not
own, and writes their cursors and the focus, none of which a plugin pane may do.
`tests/global_search_pane_gap.rs` holds that verdict as probes, so whoever closes
one of its blockers is told to revisit it.

The tasks row is the first on which a **handover** was attempted with keys in the
way, and PHASE4 §15 is the record: its rendering is now reproduced to the frame
(the plugin's copy scrolls with the cursor, ADR-38), and eight of its ten keys
need a power a plugin pane is not given. That was a second blocker beside
ADR-37's, and a different kind — the build blocker was one release decision, taken
since (ADR-40); this one is a question about what an installed plugin may do to the
interface, and it is now the binding constraint. It is step 8 of §4's worklist.

The code-review row is the first that is **partly** filled: the bundled plugin
reproduces the unified diff stream's lines and nothing else, and §11 of the same
document itemises the rest of the view — the paired layout, the headers, the
comments, the marks, the find sub-mode, the target picker, the footer and the
compose box — with the reason each is unported. That row therefore needs both a
handover *and* the remaining surface before `src/ui/code_review.rs` can go, which
is a longer list than any other pane's.

The automations row is the first where a port's **keys ship** (PHASE4 §17,
ADR-41): a plugin holding `input` keeps a cursor of its own across renders, so five
of the pane's seven keys act on the row it drew, through `automations-write` — which
had no consumer before. That narrows the tasks blocker rather than closing it: what
remains unportable is a key that writes **kernel** view state (a cursor thurbox
owns, a focus, an active session), plus record creation, text authoring, a central
seat, a modal, and reaching an agent. The pane's own circular wrap with the session
list is on that list, and stays the kernel's — a wrap is a claim about adjacency,
and the reproduction cannot be seated adjacent to anything (`PaneSlot` names only
the right column).

That row's handover has since been attempted and refused (ADR-43), and the refusal
moved the reason: not the keys, but the **central seat**. Focusing the native pane
is what draws the automation editor and its run history in the central pane, and
`App::render_central_pane` branches on the pane's three *native* focuses — so a
plugin pane taking focus removes both surfaces. `n` and `Enter`/`e` are not two
keys short of parity; they are the pane's whole authoring surface, and it needs a
seat rather than a grant. The session list's row was refused the same day for the
opposite reason: its keys move the **active session**, which is kernel view state
no capability writes.

**A pane's row is ready only on handover, not on existence.** Six panes now show
why the distinction is load-bearing rather than pedantic: each plugin exists and
reproduces its pane (four exactly, code review in the part it declares, the
session list in its rows), while the native renderer is still what the interface
draws. Deleting `src/ui/info_panel.rs` today would remove the pane every user is
looking at. So `tests/teardown_gate.rs`'s pane probe is a four-way conjunction, and
`a_reproduced_pane_is_not_a_replaced_one` pins that reasoning so it cannot be
"simplified" back to a directory check.

**And a third condition, because the first two together permitted the mistake**
(ADR-37): the runtime that draws the replacement must reach the build a user
installs. A bundled pane is Luau, so a build without the VM draws it as an empty
column. While `mlua` was optional (`default = []`), the `plugins` CI job asserted
the default dependency tree did not carry it and `release/workflow-invariants`
forbade `cd.yml` from enabling it — so handing a pane over would have removed it
from every install while the `--features plugins` test run stayed green. Without
the third conjunct, deleting a pane's renderer *and* its call in `src/app/view.rs`
satisfied the probe, the row would have been recorded ready, and this gate would
have stopped protecting the renderer. Because the condition is a fact about the
build, it blocked all seven rows together: one release decision, not seven pane
problems.

**Stage B has since taken that decision** (ADR-40): `Cargo.toml` reads
`default = ["plugins"]`, the CI assertion is inverted to require the runtime in
the default dependency tree, and release invariant 2 is replaced by its inverse.
So the third condition now **holds**, and it stays checked rather than retired —
`the_build_condition_holds_and_still_gates_a_handover` asserts both that it holds
and that each pane row is now blocked only by its own pane-level reason, so a
later change removing the runtime from `default` fails the gate instead of quietly
emptying every handed-over pane.

**And a fourth condition, because the first three together permit a quieter
mistake** (ADR-48): the pane's equality oracle must hold a **recorded** expectation,
not only a comparison against the native builder the deletion removes. Conditions
1-3 protect the pane — violate one and a column is empty, which someone notices.
Condition 4 protects the *evidence*: each oracle asserts the plugin's tree equals
`ui::tasks_panel::tasks_tree` or the like, so the deletion takes the right-hand side
with it and the repair that compiles is to drop the comparison. What survives is a
test that the plugin renders without erroring, and the pane looks perfect while
nothing constrains it. The recording is provable only while the native builder is
present, so unlike the other three this condition's window closes:
`a_pane_whose_oracle_is_differential_is_not_handed_over` refuses the handover, and
`every_reproduced_pane_records_its_native_tree` fails now rather than then. Global
search names no oracle — no bundled plugin, nothing to constrain, blocked by
condition 1.

Handing a pane over is therefore its own step, distinct from writing its plugin:
it means `App::view` drawing the plugin's pane in the native one's place. Which
needs the plugin pane to be reachable from the keyboard (PHASE4 §5, done), to be
seatable in the native pane's region (PHASE4 §21, done), to answer its action and
feature flag (PHASE4 §22, done), to carry a recorded oracle (PHASE4 §23, done for
all six reproduced panes), and to render on events rather than on a 1 s poll
(PHASE4 §7 and §13, and the session-list spike's third condition) — which is the
one still open.

Stage B's *exit* criterion ("at least one plugin that thurbox did not write") is a
separate matter and cannot be met until a release carrying the host has shipped —
that gates Stage C and `2.0.0`, not the handovers.

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
   does **not** unblock its unit.
7. ~~**Stage B and the Cargo default flip.**~~ **Done** (ADR-40). Listed here
   rather than after the handovers, which is where it sat until ADR-37: no pane
   can be handed over before it, because a Luau pane in a build with no Luau VM is
   a pane the user does not have. `default = ["plugins"]`, MSRV 1.88, the CI
   assertion inverted, release invariant 2 replaced by its inverse, and the
   bundled example pane seeded hidden so a fresh launch still looks like v1. Two
   of the four release targets (`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`)
   are verified only by the release build itself; ADR-40 records which and why.
   Remaining flip: `2.0.0`.
8. **A view-write channel, or the panes with keys stay kernel.** New, and it is
   not a pane's own work: five of the seven panes answer keys that move a cursor,
   take focus, scroll another pane, create a record or start a session, and
   nothing a plugin holds writes view state. PHASE4 §15 records the measurement on
   the tasks pane — where the two keys that *are* expressible still cannot name a
   row, because a plugin holds the keys only while the kernel publishes no cursor
   — and PHASE4 §16 the measurement on the **file viewer**, which is worse: not
   one of its seven keys is a record write, two of them need powers the vocabulary
   does not define at all (expanding a directory *reads* it; expanding a file
   *launches an editor*), and its `/` sub-mode's keys are fixed rather than
   rebindable, so a ported sub-mode could not meet the parity bar even in
   principle. `tests/tasks_pane_input_gap.rs` and
   `tests/file_viewer_pane_input_gap.rs` hold both verdicts as probes. Designing
   this is deciding what an installed plugin may do to the user's interface, so it
   belongs before the handovers rather than inside one.

   A consequence worth carrying into that design, found by giving a plugin's list
   a scroll track (ADR-39): a plugin pane's **thumb is an indicator, not a
   control**, because it reports a cursor the plugin does not own. Every drag,
   click-to-position and wheel gesture over a plugin pane's content is the same
   missing write.
9. **The seven handovers** — `App::view` drawing each plugin in its native pane's
   place, which on top of steps 7 and 8 needs the pane seatable in the native
   one's region, answering its action and `[features]` flag, and rendering on
   events rather than on a 1 s poll (PHASE4 §14). Only then may a native renderer
   go.

   **Each handover also needs its pane's oracle recorded first, in a change that
   does not perform the handover** (ADR-42). Every bundled pane's proof was
   *differential* — it asserts the plugin's tree equals the native builder's, and
   the native builder is what the handover deletes — so it could fail before the
   handover and not after it, leaving a test that the plugin renders without
   erroring. The recording must come from the native builder while it still exists,
   or it freezes a plugin defect as the expectation. **Done for all six reproduced
   panes** (`tests/snapshots/bundled_*__*.snap`), and no longer owed by a handover
   at all: ADR-48 moved the obligation to *reproduction* and made it the pane
   probe's fourth condition, so a pane reproduced without a recording fails the
   gate before any handover is attempted.

   One of the seven carries an extra step of its own. `src/ui/file_viewer.rs` is
   the only pane module that is its pane's **model**: `FileViewerState` lives
   there, `App` owns one, the published `files` section is derived from it, and
   the module also owns `visible_window` — the rule every *plugin* list is
   scrolled by and four other native panes window with. That pane's handover
   therefore begins by lifting its model out of `ui`, which PHASE4 §16 records and
   deliberately does not do in advance of a destination. A milder version of the
   same shape sits in the info panel's module, which also declares `SystemMetrics`
   — an *input* the metrics collector fills and `App` owns, so a relocation rather
   than a design, but a reason that deletion is not confined to `ui`.

   Two of the seven have now had a handover **attempted and refused**, and their
   remaining requirements are enforced rather than described (ADR-43): the
   **automations pane** (`tests/automations_pane_handover_gap.rs`, 10 rows) and the
   **session list** (`tests/session_list_pane_handover_gap.rs`, 9 rows). Each row
   is re-derived from the source and tagged structural / vocabulary / wiring, and
   each gate derives its verdict from its rows — so a requirement that closes for
   an unrelated reason fails the gate instead of expiring silently. The
   automations pane's deciding row is not one §17 predicted: focusing the *native*
   pane is what turns the **central** pane into the automation editor plus its run
   history, so a plugin pane taking focus removes both. The session list's is the
   spike's own second condition — the cursor stays kernel state — which is right,
   and is exactly what forbids a plugin pane from driving it.

   Five of those nineteen rows are **shared** by both panes: the left seat, the
   render trigger, a pane that is not told its own focus, and the
   module-as-model class. So step 9's remaining cost is concentrated in a few host
   decisions rather than spread across seven panes.

Nothing on this list is unblocked by deleting something first, which is the
whole finding: the teardown has no safe first step yet.
