# Flow extension — architectural improvement plan

> Status: planning document. Flow is experimental; this plan proposes the
> next round of enhancements to the flow ecosystem
> (`extensions/flow/`: `FLOW.md` spec, helper scripts, `install.sh`).
> Each item below lists the **problem**, the **proposed change**, the
> **files touched**, and the **effort / risk** so they can be picked up
> independently.

## Background: how flow works today

Flow is an agent-agnostic triage loop built entirely on `thurbox-cli`:

- A dedicated `flow` session runs `FLOW.md` (the behavior spec) on a cheap
  model. Brain-dumps become thurbox **tasks**; dispatchable tasks spawn
  **worker sessions** (`task-<id>-<slug>`) on `flow/<slug>` worktree
  branches.
- A `flow-tick` cron automation (every 5 min) re-invokes the flow session
  in TICK mode to monitor workers; workers also ping the flow session the
  moment they finish so the next task dispatches without waiting.
- Three shell helpers keep the triager to ~one shell call per mode:
  `create-task.sh` (atomic create + dispatch), `flow-snapshot.sh`
  (backlog + sessions view), `parse-result.sh` (extract the
  `===RESULT===` sentinel).

The whole control plane lives in **LLM prose + three small bash scripts**.
That is the strength (agent-neutral, no recompile) and the weakness (the
cheap triager carries policy that should be enforced in code).

## Guiding principle for these improvements

**Move policy out of prose and into the scripts.** Every rule the cheap
triager must remember by hand (capacity caps, base-branch hygiene,
staleness, result parsing) is a place it can silently get wrong. The
scripts are deterministic, testable, and free; the model should *decide*
(which repo, which worker, what priority) and *delegate mechanics* to the
helpers.

---

## P1 — High impact, low risk (do these first)

### 1.1 Enforce dispatch capacity in `create-task.sh`, not in prose

**Problem.** "max 3 running `task-*` sessions" lives only in FLOW.md. The
haiku triager has to count live sessions itself on every dispatch; if it
miscounts (or skips the snapshot), it over-spawns and thrashes the box.

**Change.** Teach `create-task.sh` (and a new `dispatch.sh`, see 1.2) a
`FLOW_MAX_WORKERS` cap (default 3). Before `task run`, count live
`task-*` sessions via `thurbox-cli session list`; if at/over cap, skip the
run and leave the task `todo`, printing `{"queued": <id>}` instead of
`{"spawned": ...}`. The triager keeps its current behavior but can no
longer over-dispatch even if it forgets.

**Files.** `scripts/create-task.sh`, `FLOW.md` (DISPATCH section: capacity
is now enforced, prose becomes "the helper queues over capacity").
**Effort.** ~1h. **Risk.** Low — additive, default cap = current prose.

### 1.2 Extract a single `dispatch.sh` and add a queue drain

**Problem.** Dispatch logic is split: `create-task.sh` runs `task run`
inline, while TICK re-implements "find eligible todos and run them" in LLM
prose. The promote-when-a-slot-frees path depends entirely on the model
remembering to re-scan on every tick.

**Change.** Add `scripts/dispatch.sh` that is the *one* place dispatch
happens: it reads `task list`, filters `status=todo` with a spawn action,
respects `FLOW_MAX_WORKERS`, and runs `task run` for as many as fit
(oldest-first, optionally priority-ordered — see 2.1). `create-task.sh`
calls it after create; TICK calls it instead of hand-rolling the scan.
Output is a compact JSON array of `{spawned|queued, id}` the triager can
echo.

**Files.** new `scripts/dispatch.sh`; `create-task.sh` and `FLOW.md`
(TICK step 2, CAPTURE step 3) call it. `install.sh` adds it to the fetch
list + permission allowlist. **Effort.** ~2h. **Risk.** Low-medium —
centralizes existing behavior; cover with a bats test.

### 1.3 Fix the base-branch default divergence

**Problem.** `thurbox-cli task create` defaults `--base main` (a *local*
branch). `create-task.sh` defaults `origin/main` only when `--worktree` is
passed *and* `--base` is omitted. If the triager passes `--worktree`
without `--base` through some other path, or calls `task create` directly,
it can base a worktree on a stale local `main`. FLOW.md is emphatic that
the base must always be `origin/<default>`.

**Change.** Make `create-task.sh` *always* normalize a bare branch base to
its `origin/` form (e.g. `--base main` → `origin/main`) and always fetch
first, and document that the helper is the only sanctioned create path.
Add a guard that refuses a non-`origin/` base unless `FLOW_ALLOW_LOCAL_BASE=1`.

**Files.** `scripts/create-task.sh`, `FLOW.md` (CAPTURE note). **Effort.**
~30m. **Risk.** Low.

### 1.4 Harden `parse-result.sh` against wrapped / multi-line JSON

**Problem.** `parse-result.sh` reads only the *single line immediately
after* `===RESULT===`. Terminal capture wraps long lines, and a worker
that pretty-prints or whose `notes` field is long will produce JSON that
spans several visual rows — the parser then sees truncated JSON and
returns exit 2 (malformed), which TICK treats as "still working" and the
finished task never closes.

**Change.** After locating the last `===RESULT===`, slurp *all* remaining
lines, strip the capture's wrap artifacts, and attempt to parse the
concatenation as one JSON object (greedy: try the longest balanced
`{...}`). Keep the exit-code contract (0/1/2). Add fixtures: a wrapped
single object, a pretty-printed object, trailing log noise after the JSON.

**Files.** `scripts/parse-result.sh`; new `scripts/parse-result.bats`
fixtures. **Effort.** ~2h. **Risk.** Medium — parsing is the crux of TICK;
must be well-tested. Mitigation: bats suite mirroring `install.bats`.

### 1.5 Make the worker→flow ping resilient

**Problem.** Every worker ends with
`thurbox-cli session send "$(thurbox-cli session list | jq -r '... name=="flow" ...')" "tick"`.
If the flow session is renamed, not yet listed, or jq errors, the ping is
lost silently and dispatch stalls until the 5-min cron. The full command
is also duplicated verbatim into every task description by the triager.

**Change.** Ship `scripts/notify-flow.sh` (installed to the flow home and
on `PATH` for workers via the seeded prompt) that resolves the flow
session id robustly (by name, falling back to the `flow`-agent session),
retries once, and is a no-op-with-clear-exit if no flow session exists.
The task-description template calls `notify-flow.sh tick` instead of the
inline subshell. Keeps a copy-paste fallback in a comment for non-flow
environments.

**Files.** new `scripts/notify-flow.sh`; `FLOW.md` (description template,
worker self-report); `install.sh` (fetch + allowlist). **Effort.** ~1h.
**Risk.** Low.

---

## P2 — Medium impact (worth doing, slightly more design)

### 2.1 Priority-aware dispatch ordering

**Problem.** `priority: high|normal|low` is captured into the description's
first line and shown in the snapshot, but dispatch is effectively
oldest-first. A high-priority task created after the queue is full waits
behind low-priority ones.

**Change.** `dispatch.sh` (1.2) parses the `priority:` line and orders
eligible todos high → normal → low, then by age. Cheap, no schema change
(priority stays in the description). Optionally surface a stable sort key.

**Files.** `scripts/dispatch.sh`, `scripts/flow-snapshot.sh` (already shows
priority). **Effort.** ~1h. **Risk.** Low.

### 2.2 Worker staleness / wedged-session detection with a timeout

**Problem.** TICK detects a *missing* worker session (stale → reset to
todo) but not a *wedged* one: a worker stuck on a permission prompt, an
infinite loop, or a crashed-but-still-open tmux window shows no sentinel
and no missing session, so it sits `in_progress` forever, holding a
capacity slot.

**Change.** Record a dispatch timestamp (task `updated_at` already flips on
dispatch; or write a sidecar `~/flow/state/<id>.dispatched` epoch). In
TICK, a worker older than `FLOW_WORKER_TIMEOUT` (default 90 min) with no
sentinel is surfaced under "Needs you" with its last 10 lines, and after a
second timeout window is auto-reset to todo (freeing the slot). Make the
auto-reset opt-in via `FLOW_AUTO_RESET=1` to start conservative.

**Files.** `FLOW.md` (TICK step 1), new `scripts/worker-state.sh` helper,
`install.sh`. **Effort.** ~3h. **Risk.** Medium — must not reset a worker
that is legitimately slow; default to *surface, don't reset*.

### 2.3 Machine-readable repo routing (`repos.toml` alongside `repos.md`)

**Problem.** `repos.md` is a free-form markdown table parsed by the LLM on
every mode. Keyword routing is therefore non-deterministic and the model
re-reads/parses a doc each call. A typo'd path silently misroutes a task.

**Change.** Add an optional `repos.toml` (name → path, base, keywords[])
that the scripts can parse deterministically. `create-task.sh` gains a
`--repo-name <name>` that resolves path + base from `repos.toml` (validated
to exist + be a git repo). Keep `repos.md` as the human-editable doc;
generate/validate `repos.toml` from it with a small helper, or treat
`repos.toml` as canonical and `repos.md` as docs. The triager still does
fuzzy keyword → name matching (its real strength), but path/base resolution
becomes deterministic and validated.

**Files.** new `scripts/resolve-repo.sh`, `create-task.sh`, `FLOW.md`
(CAPTURE repo step), `install.sh` (seed `repos.toml`). **Effort.** ~3h.
**Risk.** Medium — adds a second source of truth; mitigate by making one
generate the other.

### 2.4 Structured TICK output / lightweight metrics

**Problem.** There is no record of flow's throughput: tasks dispatched per
day, failure rate, mean time-in-flight, how often workers wedge. Hard to
know if flow is helping or thrashing.

**Change.** Append one JSON line per dispatch/completion to
`~/flow/state/metrics.jsonl` (event, task id, agent, epoch). Add a
`scripts/flow-report.sh` (or extend `status`) that summarizes the last N
days. Pure-local, no new deps; rotate/cap the file.

**Files.** new `scripts/flow-report.sh`, hooks in `create-task.sh` /
`dispatch.sh`, `FLOW.md` (REPORT can cite it). **Effort.** ~3h. **Risk.**
Low.

---

## P3 — Larger / longer-horizon (design first, may need core changes)

### 3.1 First-class task action edit (remove the remove+recreate wart)

**Problem.** FLOW.md documents a real wart: a plain todo that becomes
dispatchable can't gain an action via `task edit` (verified: `task edit`
only takes `--title/--description/--status`), so the flow must **remove +
recreate**, churning the id and breaking any external reference.

**Change.** This is a **core thurbox-cli change**, not an extension-only
one: extend `task edit` to accept the same spawn/send action flags as
`task create` (`--repo`, `--agent`, `--worktree`, `--base`, `--session`),
updating the action columns in place. Then `create-task.sh`'s "remove +
recreate" path in FLOW.md collapses to a single `task edit`.

**Files.** `src/cli/` (task edit dispatch), `storage/tasks.rs`
(`update_task` already mirrors action columns — confirm it can set them),
`FLOW.md` (DISPATCH: "a plain todo that became ready → `task edit --repo
… --agent …`"). **Effort.** ~half-day incl. tests. **Risk.** Medium —
touches the core binary; needs a CLI test. Biggest UX win for the flow
loop.

### 3.2 A `flow` subcommand namespace in thurbox-cli (optional consolidation)

**Problem.** The control plane is bash glue over `thurbox-cli`. As the
helpers grow (dispatch cap, queue drain, repo resolution, metrics), the
logic that *should* be deterministic and tested keeps living in shell.

**Change.** Consider promoting the stable helpers into a `thurbox-cli flow`
subcommand group (`flow dispatch`, `flow snapshot`, `flow parse-result`)
once their behavior settles. Keeps agent-neutrality (still data + CLI), but
the policy is Rust-tested and the install surface shrinks to "point a
session at FLOW.md." Defer until the bash helpers stabilize — premature
now while flow is experimental.

**Files.** `src/cli/` (new subcommand), `FLOW.md`, `install.sh` (fewer
scripts). **Effort.** ~1–2 days. **Risk.** Higher — core surface area;
only after the shell design is proven.

### 3.3 PR-state-aware completion

**Problem.** Workers self-report `pr_url` in the sentinel, but flow never
checks PR state. A worker can report `ok` with an open PR that then fails
CI; flow has already closed the task and moved on.

**Change.** When a sentinel carries `pr_url`, optionally poll PR/CI state
(`gh pr view --json state,statusCheckRollup`) in TICK and surface a failing
or still-open PR under "Needs you" rather than silently closing. Gated on
`gh` being present; degrades to current behavior without it.

**Files.** `FLOW.md` (TICK), new `scripts/pr-state.sh`. **Effort.** ~3h.
**Risk.** Low-medium (depends on `gh`; must degrade cleanly).

---

## Suggested sequencing

1. **P1 batch** (capacity cap, `dispatch.sh`, base-branch fix, robust
   `parse-result`, resilient notify) — all extension-only, low risk, each
   removes a class of silent triager error. Land with a `bats` suite for
   the scripts mirroring `install.bats`.
2. **P2 batch** (priority ordering, staleness timeout, `repos.toml`,
   metrics) — once dispatch is centralized in `dispatch.sh`, these slot in
   cleanly.
3. **P3** — `task edit` action support (3.1) is the one core change with a
   clear, high payoff; do it standalone. Defer 3.2/3.3 until the shell
   design has stabilized in real use.

## Testing strategy (applies throughout)

- Add `extensions/flow/scripts/*.bats` mirroring `scripts/install.bats`:
  fixtures for `parse-result.sh` (wrapped/pretty/noisy), `dispatch.sh`
  (capacity boundaries, priority order), `resolve-repo.sh` (missing repo,
  non-git path). Wire into CI next to the existing bats hook.
- Keep every helper a pure function of its inputs (stdin / flags / files)
  so it is unit-testable without a live thurbox DB; the few that must hit
  `thurbox-cli` should isolate that call behind one function for stubbing.

## Non-goals / explicitly out of scope

- No change to flow's agent-neutrality: the triager and workers stay plain
  `agents.toml` entries; nothing here pins a model or CLI.
- No persistent flow daemon: the cron tick + worker ping model stays.
- No external task-source sync (Jira/GitHub Issues) — the `Task` scaffolding
  exists in core but is out of scope for the flow extension itself.
