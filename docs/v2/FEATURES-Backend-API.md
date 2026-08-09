# Thurbox v2 — Plugin Backend

A plugin has two halves. [FEATURES-View-Tree.md](FEATURES-View-Tree.md) specifies the **view**
half — what a plugin draws while the TUI is running. This document specifies
the **service** half: what a plugin *does*, including when no TUI exists.

```text
plugin/
├── plugin.toml
└── src/
 ├── service.luau ← this document — runs headless, owns data + integrations
 └── view.luau ← FEATURES-View-Tree.md — runs only with the TUI, render + input
```

Either half may be omitted. A CI-status integration that only syncs data is
service-only. A pane that renders kernel state is view-only.

---

## 1. Why the split exists

v1 guarantees that **automations fire with the TUI closed** (`docs/ARCHITECTURE.md`
ADR-8b): a detached tmux heartbeat keeper loops `thurbox-cli automation tick`
every 60 s, with optional systemd/launchd units for reboot-proof firing. A
plugin system whose backend only lived inside the TUI process would silently
revoke that guarantee — a task-sync or CI-watch plugin would stop working the
moment you quit.

So the seam is drawn where thurbox's own seam already is
([ADR-V16](ARCHITECTURE.md#adr-v16)):

| Half | Hosted by | Lives while |
|---|---|---|
| **service** | The TUI when running; otherwise the heartbeat keeper or a `thurbox-cli` invocation | thurbox is *installed* |
| **view** | The TUI only | thurbox is *on screen* |

Three consequences worth internalizing:

1. **A service must not assume a UI.** `ctx.ui.status(...)` is a no-op
 headless. Report through logs, run history, and notifications instead.
2. **The two halves are separate VMs on separate threads** and share no state.
 They talk over a typed contract the kernel routes (§9).
3. **Capabilities are granted per half.** A view rarely needs `net` or
 `shell`; a service rarely needs anything else.

---

## 2. Entry point

```lua
local thurbox = require("@thurbox")

return thurbox.defineService({
    init = function(ctx)
        ctx.db.migrate({
            `CREATE TABLE runs (id TEXT PRIMARY KEY, repo TEXT, state TEXT, seen_at INTEGER)`,
            `CREATE INDEX runs_repo ON runs (repo)`,
        })
    end,

    services = {
        poll = function(ctx, signal)
            while not signal.aborted do
                syncOnce(ctx)
                ctx.sleep(60_000, signal)
            end
        end,
    },

    schedules = {
        ["nightly-prune"] = { cron = "0 3 * * *", run = function(ctx) prune(ctx) end },
    },

    commands = {
        ["ci.refresh"] = function(ctx) return { synced = syncOnce(ctx) } end,
    },

    cli = {
        status = {
            summary = "Show CI status for every watched repo",
            run = function(ctx, argv)
                ctx.out.table(ctx.db.all(`SELECT * FROM runs`))
            end,
        },
    },

    dispose = function(ctx) end,
})
```

`init` runs once per activation. Everything else is registration: the kernel
supervises services, fires schedules, routes commands, and dispatches CLI
verbs.

---

## 3. Hosting and lifecycle

A service VM is created by whichever host needs it first:

| Host | Starts a service when |
|---|---|
| TUI | An activation event fires ([ADR-V15](ARCHITECTURE.md#adr-v15)) |
| Heartbeat keeper (`automation tick`, 60 s) | The plugin declares `services` or `schedules` and is enabled |
| `thurbox-cli` | A plugin CLI verb or command is invoked |

**Exactly one service instance per plugin per machine.** The kernel enforces
this with an advisory lock in the database, so a running TUI and a heartbeat
tick never both run a plugin's `poll` loop. If the TUI starts while the
heartbeat already hosts the service, the service **stays with the heartbeat**
— migrating live work would break it in flight for no benefit.

A CLI invocation is the exception: it creates an **ephemeral, service-less**
VM that runs only the requested verb, with `services` and `schedules`
suppressed. This keeps `thurbox-cli ci status` fast and side-effect-free.

Lifecycle otherwise mirrors [PLUGIN-API §4](FEATURES-Plugin-API.md#4-lifecycle):
lazy activation, backoff on fault, `dispose()` with a grace period. Two
service-specific rules:

- **Reload drains first.** Running services are aborted via their `signal` and
 joined (2 s grace) before the replacement VM starts, so a reload never runs
 two copies of a poll loop.
- **A crashing service does not disable the plugin's view.** The halves fault
 independently; a broken sync shows a degraded pane, not a missing one.

---

## 4. CLI verbs

A plugin owns a namespace under `thurbox-cli`:

```toml
[[cli]]
name = "ci"
summary = "CI status and controls"
```

```lua
cli = {
    status = { summary = "Show CI status", run = function(ctx, argv) --[[ … ]] end },
    retry = { summary = "Retry a failed run", args = { id = "string" }, run = retryRun },
}
```

```bash
thurbox-cli ci status
thurbox-cli ci retry --id 4821
```

Output goes through `ctx.out`, which respects thurbox's existing convention —
human-readable on a TTY, JSON when piped, forced by `--json` / `--pretty` /
`--text`:

```lua
ctx.out.table(rows)             -- formats per the active output mode
ctx.out.json({ synced = 12 })   -- structured only
ctx.out.line("12 runs synced")  -- human only
```

Name collisions with kernel subcommands (`session`, `task`, `automation`,
`message`, `plugin`, `command`, `config`, …) are rejected at install time, not
at runtime.

**Why this rather than only `command run`**: `thurbox-cli command run ci.refresh
--json '{}'` works and is agent-friendly, but it is not a CLI a human wants to
type or a shell script wants to read. Commands are the *machine* surface
([FEATURES-Agent-API.md](FEATURES-Agent-API.md)); verbs are the human one, and a
plugin that owns real functionality deserves both.

---

## 5. Services (supervised background work)

```lua
services = {
    poll = function(ctx, signal) --[[ long-running ]] end,
}
```

| Property | Behavior |
|---|---|
| Start | On activation, after `init` resolves |
| Abort | `signal` fires on reload, disable, deactivate, and shutdown |
| Restart | On unexpected return or throw, with backoff (1s, 2s, 4s, … capped 60 s) |
| Grace | 2 s after abort before the thread is terminated |
| Concurrency | One instance per named service, machine-wide |

Services are for **watching** — polling an API, tailing a file, holding a
websocket. They are not for scheduled work; use §6.

`ctx.sleep(ms, signal)` is provided because a bare busy-wait cannot be
cancelled and would hold the service alive past abort.

---

## 6. Schedules

```lua
schedules = {
    ["nightly-prune"] = { cron = "0 3 * * *", run = function(ctx) prune(ctx) end },
}
```

Schedules are **claimed through the kernel's existing automation machinery**,
not run by a timer inside the plugin. That means they inherit ADR-8b's
properties for free: they fire from the TUI, the heartbeat keeper, or a
systemd/launchd unit; `claim_due_automation`'s atomic compare-and-swap
guarantees exactly one firer; and each run is recorded in the run history
where the automations pane already displays it.

Choosing between the two:

| Use | When |
|---|---|
| `schedules` | Work happens *at a time* — nightly prune, 15-minute sync |
| `services` | Work happens *continuously* — a poll loop, a socket, a file watch |

A `schedules` entry is preferable whenever it fits, because it costs nothing
while idle: no VM is resident between firings.

---

## 7. Storage

Two tiers, both inside the kernel database
([ADR-V17](ARCHITECTURE.md#adr-v17)).

### 7.1 Key-value

```lua
ctx.kv.set("cursor", { page = 3 })
local cursor = ctx.kv.get("cursor")
ctx.kv.list("repo:")  -- prefix scan
```

For cursors, caches, and small blobs. No schema, no migration.

### 7.2 Namespaced tables

For relational data, a plugin declares migrations and the **kernel executes
and versions them**:

```lua
ctx.db.migrate({
    `CREATE TABLE runs (id TEXT PRIMARY KEY, repo TEXT, state TEXT)`,  -- v1
    `ALTER TABLE runs ADD COLUMN url TEXT`,                            -- v2
})
```

Rules the kernel enforces:

- Every table is created in the plugin's namespace — a declared `runs` becomes
 `plugin_ci_runs`. A statement referencing anything outside the namespace is
 **rejected**, so a plugin cannot read or write kernel tables or another
 plugin's data through SQL.
- The applied version is recorded in `plugin_migrations(plugin, version)`.
 Migrations are append-only: editing an already-applied statement is a load
 error, not a silent divergence.
- `plugin uninstall --purge` drops the namespace. No orphaned tables.

Queries are parameterized only:

```lua
local rows = ctx.db.all(`SELECT * FROM runs WHERE repo = ?`, { repo })
ctx.db.run(`UPDATE runs SET state = ? WHERE id = ?`, { state, id })
ctx.db.tx(function(t) --[[ … ]] end)  -- transaction
```

There is no raw connection handle, which is the deliberate difference from the
usual JS-ecosystem answer of handing a plugin a live `better-sqlite3`
`Database`. Keeping execution kernel-side is what preserves a single
`SCHEMA_VERSION` owner, keeps multi-instance `PRAGMA data_version` sync
coherent, and makes uninstall complete
([ADR-V17](ARCHITECTURE.md#adr-v17)).

**Kernel tables** (`sessions`, `tasks`, `automations`, `session_messages`, …)
remain reachable only through typed host APIs, never SQL.

---

## 8. Event bus

```lua
ctx.bus.publish("ci:status", { repo = repo, state = state })
ctx.bus.subscribe("flow:dispatch", function(payload) --[[ … ]] end)
```

Named channels brokered by the kernel. Both halves of a plugin can use them,
and — with the `bus` capability — **so can other plugins**.

This is the second cross-plugin composition primitive, alongside
`sessionDecorations`. It is intentionally weaker than a direct call: channels are
fire-and-forget, payloads are opaque JSON, there is no request/response, and a
subscriber cannot tell whether a publisher exists. Two plugins can therefore
cooperate when both opt in, without either being able to break the other by
changing an interface.

Channel names are namespaced by convention (`<plugin>:<topic>`) and by rule for
publishing: a plugin may publish only on its own prefix, and may subscribe to
any.

---

## 9. Talking to the view half

The halves share a typed contract:

```lua
-- src/types.luau
export type Contract = {
    listRuns: (args: { repo: string }) -> { Run },
    retry: (args: { id: string }) -> (),
}
```

```lua
-- src/service.luau
rpc = {
    listRuns = function(ctx, args)
        return ctx.db.all(`… WHERE repo = ?`, { args.repo })
    end,
}
```

```lua
-- src/view.luau
local runs = ctx.service.listRuns({ repo = repo })
```

The kernel routes and validates against the shared declaration, so both halves
type-check against one source. The two halves are separate VMs on separate
threads, so the call yields the caller's coroutine and is deadline-bounded; a
view must therefore fetch into state and render from state
([LIMITATIONS §4.2](LIMITATIONS.md#42-no-synchronous-host-calls-during-render---by-design)).

A service may also push to its view (`ctx.view.notify(event)`), which is how a
finished sync updates a pane without polling.

---

## 10. Control-socket methods

A plugin can expose methods on thurbox's control socket
([AGENT-API §6](FEATURES-Agent-API.md#6-control-socket)):

```lua
socket = { ["ci.summary"] = function(ctx) return summarize(ctx) end }
```

These are for external tooling and scripts that want a persistent connection
rather than a `thurbox-cli` invocation per call. They are **not** a general HTTP surface —
thurbox has no HTTP server, and adding one is a separate decision with its own
security story. A plugin cannot open a listening port through this API, and
nothing here is reachable off the machine
([FEATURES-Agent-API.md §8](FEATURES-Agent-API.md#8-non-goals-for-v20)).

---

## 11. Spawn contributions

v1 extensions could patch an agent's argv (`[[agent_patches]]`), and dropping
that outright would be a real regression (see
[LIMITATIONS §4.5](LIMITATIONS.md#45-no-middleware)). A bounded form is restored:

```lua
spawn = {
    contribute = function(ctx, spawn)
        return {
            env = { CI_TOKEN_FILE = "/run/secrets/ci" },
            args = { "--settings", `{ctx.home}/ci.json` },
        }
    end,
}
```

Hard limits, so this stays a contribution and not middleware:

- **Append only.** It cannot remove or rewrite existing args or env.
- **No veto.** It cannot cancel or redirect a spawn.
- **Fail-open with a 500 ms deadline.** A slow or throwing contributor is
 skipped and logged; the session still spawns.
- **Conflicts are visible.** Two plugins setting the same env key is a logged
 warning and last-declared-wins by manifest order, never a silent surprise.
- **Dangerous env keys are refused.** `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`,
 `GIT_SSH_COMMAND`, `GIT_EXTERNAL_DIFF`, `BASH_ENV`, `NODE_OPTIONS` and the
 like turn a contribution into arbitrary code execution inside every agent
 session; `PATH` may only be **prepended** with paths under `{plugin_data}`.
 Rejections are logged and listed by `thurbox plugin doctor`
 ([SECURITY.md §8](SECURITY.md#8-spawn-contributions)). Append-only bounds
 *who wins*, not *what can be injected* — this bounds the latter.

The same shape carries the v1 `hook_schema` capability: a plugin contributing
an agent declares which hook family that agent speaks, and status wiring
follows.

---

## 12. Skills

```toml
[[skills]]
path = "skills/ci"
```

A skill is agent-facing markdown shipped next to the code it describes — the
missing half of [FEATURES-Agent-API.md](FEATURES-Agent-API.md). A JSON Schema
tells an agent a command's *shape*; a skill tells it *when and why* to reach for
it. v1 already ships this pattern by hand: `extensions/flow/FLOW.md` is a
behavior spec surfaced to whichever CLI runs it through context-file symlinks.
Skills make it a first-class, agent-neutral registry instead of a symlink
convention.

Skills register in a kernel registry and surface two ways:

```bash
thurbox-cli skill list # what this thurbox can teach an agent
thurbox-cli skill show ci # the markdown
```

and, for agents that auto-discover context files, optional materialization
into a session's workspace. Which files those are is agent-specific, so the
kernel stays neutral: it exposes the registry and materializes on request,
rather than knowing that one CLI reads `CLAUDE.md` and another reads
`AGENTS.md`.

---

## 13. Settings

Typed descriptors, declared in the manifest and readable in both halves, with
change notification in the service:

```lua
ctx.settings.get()  -- typed values
ctx.settings.onChange(function(next, prev) --[[ … ]] end)
```

Values live in the plugin's namespace and render in the Settings panel under
the plugin's name. A service that caches a derived value from settings should
rebuild it in `onChange` rather than at `init` only.

---

## 14. Capabilities used by this half

The canonical capability table is
[PLUGIN-API §8](FEATURES-Plugin-API.md#8-capability-reference) — one list, one
place, covering both halves. Seven entries in it exist for the service half:

| Capability | Values | Grants |
|---|---|---|
| `db` | `none` \| `kv` \| `tables` | KV only (§7.1), or namespaced tables with migrations (§7.2) |
| `cli` | `bool` | Registering CLI verbs (§4) |
| `services` | `bool` | Long-running supervised work (§5) — implies a resident VM |
| `schedules` | `bool` | Cron entries in the kernel scheduler (§6) |
| `bus` | `none` \| `own` \| `all` | `own` = publish on its own prefix; `all` = also subscribe to other plugins' channels (§8) |
| `socket` | `bool` | Exposing control-socket methods (§10) |
| `spawn` | `none` \| `contribute` | Appending env/args at session spawn (§11) |

`services` is the one to scrutinize at install time: it is the difference
between a plugin that runs when you look at it and a plugin that runs always.
`thurbox plugin install` calls it out explicitly.

Grants are **per half**. A view rarely needs `net` or `shell`; a service never
needs `pty`. Declaring them separately is what keeps the install prompt
meaningful instead of a union of everything either half might want.

---

## 15. Budgets and failure

| Concern | Limit |
|---|---|
| `init` | 10 s, then the plugin faults |
| Command / cross-half call | 250 ms soft (status shown), 10 s hard |
| Spawn contribution | 500 ms, fail-open |
| Service restart | Backoff to 60 s; 10 consecutive faults disables the service |
| Schedule run | Recorded in run history with exit state and tail-truncated output |
| Resident memory | Reported per plugin by `thurbox plugin doctor` |

A service-half fault is surfaced where its consequences are, not only in a
log: a degraded badge on the plugin's pane, an entry in `plugin doctor`, and —
if the plugin declares `notify` — an OS notification for repeated failure.

---

## 16. Worked example

A CI-watch plugin, end to end. Service-only until the last two lines.

```toml
# plugin.toml
name = "ci"
entry_service = "src/service.luau"
entry_view = "src/view.luau"

[capabilities]
db = "tables"
net = ["api.github.com"]
cli = true
services = true
bus = "own"
sessions = "read"

[[panes]]
id = "ci"
slot = "right"
title = "CI"

[[cli]]
name = "ci"
summary = "CI status and controls"

[[skills]]
path = "skills/ci"

activation = ["onStartup"]
```

```lua
-- src/service.luau
local thurbox = require("@thurbox")

return thurbox.defineService({
    init = function(ctx)
        ctx.db.migrate({
            `CREATE TABLE runs (id TEXT PRIMARY KEY, repo TEXT, branch TEXT, state TEXT)`,
        })
    end,

    services = {
        poll = function(ctx, signal)
            while not signal.aborted do
                for _, s in ctx.sessions.list() do
                    local run = fetchRun(ctx, s.repo, s.branch)
                    ctx.db.run(`INSERT OR REPLACE INTO runs VALUES (?,?,?,?)`,
                        { run.id, s.repo, s.branch, run.state })
                    ctx.bus.publish("ci:status", { session = s.id, state = run.state })
                end
                ctx.sleep(60_000, signal)
            end
        end,
    },

    rpc = { listRuns = function(ctx) return ctx.db.all(`SELECT * FROM runs`) end },
    cli = { status = { summary = "Show CI status", run = showStatus } },
})
```

What this gets for free, and would not get from a view-only plugin: it keeps
syncing with the TUI closed, it owns `thurbox-cli ci status`, its failures land
in run history, and any other plugin can react to `ci:status` — including
decorating session rows it does not own.

---

## 17. What this closes, and what it does not

The service half is what stops "plugin" meaning "a pane". It gives a plugin
plugin-owned CLI verbs (§4), supervised background work (§5), scheduled work
that inherits ADR-8b (§6), relational storage (§7), pub/sub (§8), a typed
channel to its own view (§9), spawn contributions (§11), and agent-facing
skills (§12) — the set of things v1 extensions could reach for and v1 *panes*
could not.

Still deliberately absent, with reasons:

| Not provided | Why |
|---|---|
| Raw SQLite handle | One schema owner; clean uninstall (§7.2) |
| HTTP routes | thurbox has no HTTP server (§10) |
| Spawn veto or arg rewriting | Middleware; contributions are append-only (§11) |
| Direct plugin-to-plugin calls | Interface coupling; the bus is the sanctioned path (§8) |
| Access to another plugin's tables | Namespace enforcement (§7.2) |
