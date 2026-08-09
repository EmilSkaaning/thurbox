# Thurbox v2 — Agent API

Thurbox already puts a coding agent inside every session. v2 makes the
orchestrator itself **operable by those agents**: every plugin command is
discoverable, typed, and callable from inside a session
([ADR-V10](ARCHITECTURE.md#adr-v10)).

This is the piece that turns thurbox from "a TUI that agents run inside" into
"an orchestrator agents can drive". v1 already has the coordination patterns
(`docs/ORCHESTRATION.md`) but they run on `thurbox-cli` invocations, the message
queue, and screen conventions; a typed command registry makes them ordinary tool
calls (§7).

---

## 1. One registry, four entry points

A command is declared once in a plugin manifest and reachable four ways:

```text
 ┌──────────────────────┐
 │ command registry │
 │ id · schema · perms │
 └──────────┬───────────┘
 ┌─────────────┬─────────┴────────┬──────────────┐
 ▼ ▼ ▼ ▼
 keybinding command palette thurbox-cli control socket
 (user) (user) (scripts, (agents inside
 agents) sessions)
```

The registry is populated from manifests **without starting any plugin
process**, so discovery works even for suspended or not-yet-activated plugins.
Invoking a command activates its plugin on demand.

---

## 2. Discovery

```bash
$ thurbox-cli command list --json
[
 {
 "id": "tasks.create",
 "plugin": "tasks",
 "title": "New task",
 "description": "Create a task in the todo list",
 "args": {
 "type": "object",
 "properties": {
 "title": { "type": "string" },
 "description": { "type": "string" }
 },
 "required": ["title"]
 },
 "returns": { "type": "object", "properties": { "id": { "type": "string" } } },
 "agent_callable": true
 }
]
```

The `args` schema is JSON Schema, which is deliberate: it is the same shape
agent CLIs already consume for tool definitions, so an agent can be handed the
command list as a toolset with no translation layer.

```bash
thurbox-cli command list # human table
thurbox-cli command list --plugin tasks # scope to one plugin
thurbox-cli command describe tasks.create # full schema + examples
```

---

## 3. Invocation

```bash
# Positional/flag form, validated against the schema
thurbox-cli command run tasks.create --title "Fix flaky test" --description "…"

# JSON form, for structured callers
thurbox-cli command run tasks.create --json '{"title":"Fix flaky test"}'

# Result on stdout (JSON when piped, human when a TTY — same rule as v1)
{"id": "t_01H…", "status": "todo"}
```

Errors are structured and non-zero exit:

```json
{ "error": "E_ARGS", "message": "missing required argument: title", "command": "tasks.create" }
```

| Code | Meaning |
|---|---|
| `E_ARGS` | Arguments failed schema validation |
| `E_UNKNOWN_COMMAND` | Not in the registry |
| `E_PLUGIN_UNAVAILABLE` | Plugin faulted or disabled |
| `E_CAPABILITY` | Command needs a capability the plugin was not granted |
| `E_DENIED` | Caller is not permitted to invoke this command (§5) |
| `E_TIMEOUT` | Exceeded the command deadline |

---

## 4. Identity — an agent knows who it is

v1 already injects `THURBOX_SESSION` (the session id) and `THURBOX_TASK` into
every spawned agent process, which is what lets `thurbox-cli message send`
stamp provenance with no ids passed. v2 extends the same mechanism to
commands:

```bash
# Run from inside a session's agent — no ids needed anywhere
thurbox-cli command run review.open # reviews THIS session's worktree
thurbox-cli command run tasks.create --title "…" # attributed to this session/task
```

The kernel resolves `{session}` and `{task}` argument defaults from the
caller's injected environment. An agent therefore operates thurbox in terms of
*what it wants*, never in terms of ids it would have to scrape.

---

## 5. Permission model

Two orthogonal gates, both required:

1. **Can the plugin do this at all?** The plugin's manifest capabilities
 ([PLUGIN-API §8](FEATURES-Plugin-API.md#8-capability-reference)). A command in a
 plugin without `sessions = "control"` cannot delete a session no matter who
 invokes it.
2. **May this caller invoke it?** Per-command caller policy:

```toml
[[commands]]
id = "sessions.delete"
title = "Delete session"
agent_callable = true
agent_policy = "confirm" # allow | confirm | deny
```

| Policy | Behavior when invoked from inside a session |
|---|---|
| `allow` | Runs immediately |
| `confirm` | Queues a prompt in the TUI; the CLI call blocks (with `--timeout`) until the user answers |
| `deny` | `E_DENIED` — user-only, e.g. `plugin.install` |

Defaults are conservative for destructive verbs: anything that deletes,
force-pushes, or installs defaults to `confirm` or `deny`, and a plugin must
opt *down* explicitly. `thurbox-cli command list` reports each command's
policy so an agent can tell in advance which calls will block on a human.

**Loop guard.** Commands invoked from inside a session carry a depth counter;
a command that spawns a session whose agent invokes the same command is
stopped at depth 3 with `E_DENIED`. Without it, a self-driving orchestrator
can trivially fork-bomb itself.

---

## 6. Control socket

For callers that need lower latency or streaming than process-per-call, the
kernel exposes the same registry over a local socket
(`$XDG_RUNTIME_DIR/thurbox/control.sock`, or a named pipe on Windows). This is
the only place v2 has a wire protocol at all — plugins are in-process
([ADR-V2](ARCHITECTURE.md#adr-v2)) and cross no socket — so it defines its own
framing: JSON-RPC 2.0, one object per line, newline-delimited.

```json
{"jsonrpc":"2.0","id":1,"method":"command/run","params":{"id":"tasks.create","args":{"title":"…"}}}
```

Additional socket-only methods:

| Method | Purpose |
|---|---|
| `command/list` | Registry snapshot |
| `event/subscribe` | Stream kernel events (session status, task changes) |
| `state/query` | Read kernel state without a command round trip |

`thurbox-cli` uses the socket when a TUI is running and falls back to direct
database access when it is not — the same dual path v1 already has for
headless operation. Commands whose plugin is not running are unavailable
headlessly and report `E_PLUGIN_UNAVAILABLE`, which is why kernel-table
operations (tasks, automations, messages, sessions) remain kernel APIs rather
than plugin commands ([ADR-V9](ARCHITECTURE.md#adr-v9)).

---

## 7. What this enables

The v1 orchestration patterns (`docs/ORCHESTRATION.md`) currently coordinate
through `thurbox-cli` plus the message queue plus screen conventions. With a
typed command surface they become ordinary tool calls:

- A lead agent opens a review on a worker's session and reads the diff
 summary, instead of asking the user to look.
- A CI-watching plugin dispatches a fixer session, and the fixer's agent marks
 the task done and closes its own pane.
- A worker asks for input by queuing a `confirm` command; the answer routes
 back through the same call rather than through a mailbox convention.
- Any agent can enumerate what this thurbox can do (`command list`) and adapt,
 rather than being told in a prompt.

---

## 8. Non-goals for v2.0

- **No remote control surface.** The socket is local-only. Driving another
 machine's thurbox is a separate design with its own auth story.
- **No per-agent identity or ACLs.** Policy is per command, not per agent.
- **No transactional composition.** Commands are individual calls; there is no
 batch/rollback primitive.
- **No streaming command results.** A command returns once. Long-running work
 belongs in an automation or a spawned session.
