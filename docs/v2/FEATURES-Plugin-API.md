# Thurbox v2 — Plugin API

The contract between the Rust kernel and a plugin VM. This document is
intended to be sufficient to write a working plugin without reading kernel
source; the rendering contract it references is specified separately in
[FEATURES-View-Tree.md](FEATURES-View-Tree.md).

---

## 0. Quickstart

The smallest thing that is a plugin, end to end. Two files:

```toml
# hello/plugin.toml
name = "hello"
version = "0.1.0"
entry_view = "src/view.luau"
activation = ["onPaneVisible:hello"]

[[panes]]
id = "hello"
title = "Hello"
slot = "right"
default_visible = true
```

```lua
-- hello/src/view.luau
local thurbox = require("@thurbox")
local ui = thurbox.ui

type State = { n: number }

return thurbox.definePlugin({
    init = function(ctx): State
        return { n = 0 }
    end,

    render = function(ctx, s: State)
        return ui.column({ padding = 1 }, {
            ui.text({ content = `pressed {s.n} times`, style = { fg = "accent" } }),
        })
    end,

    update = function(ctx, s: State, e)
        if e.type == "key" then return { n = s.n + 1 } end
        return s
    end,
})
```

```bash
thurbox plugin dev ./hello    # loads it, watches the file, reloads on save
```

That is a real pane: themed, focusable, in the `Ctrl+L` focus ring, with a
`hello.hello.toggle` command the kernel generated from the manifest and a row in
the F1 editor. It declares **no capabilities**, so it can draw and receive its
own input and nothing else — which is the default
([§8](#8-capability-reference)).

From here: [§2](#2-manifest-plugintoml) for what else the manifest can declare,
[FEATURES-View-Tree.md](FEATURES-View-Tree.md) for what else `render` can
return, and [FEATURES-Backend-API.md](FEATURES-Backend-API.md) when the plugin
needs to do something while the TUI is closed.

---

## 1. What a plugin is

A directory containing a manifest and up to two Luau entry points — a
**service** half and a **view** half ([ADR-V16](ARCHITECTURE.md#adr-v16)):

```text
my-plugin/
├── plugin.toml # manifest — identity, capabilities, contributions
├── src/
│ ├── service.luau # runs headless too — FEATURES-Backend-API.md
│ ├── view.luau # runs only with the TUI — FEATURES-View-Tree.md
│ └── types.luau # type definitions shared by both halves
├── skills/ # optional — agent-facing markdown
└── README.md
```

**Either half may be omitted.** A tracker-sync integration is service-only and
draws nothing; a pane over kernel state is view-only and runs only while the
TUI does. The halves are separate VMs on separate threads, with separate
capability grants, and they fault independently.

Install locations, in resolution order:

| Location | Purpose |
|---|---|
| `$THURBOX_PLUGIN_PATH` (colon-separated) | Development override |
| `~/.config/thurbox/plugins/<name>/` | User-installed |
| Embedded in the binary | Bundled plugins (materialized to a cache dir on first run) |

The first match by name wins, so a user can shadow a bundled plugin with their
own fork by name alone.

---

## 2. Manifest (`plugin.toml`)

```toml
name = "tasks" # unique id; [a-z0-9-]+
version = "2.0.0"
description = "Todo list with agent dispatch"
entry_service = "src/service.luau" # optional — omit for a view-only plugin
entry_view = "src/view.luau" # optional — omit for a headless plugin
min_thurbox = "2.0.0" # soft gate — warn, never block
license = "MIT"
activation = ["onPaneVisible:tasks", "onCommand:tasks.create"] # §2b

# Canonical list in §8. A flat [capabilities] grants both halves; add
# [capabilities.view] / [capabilities.service] to narrow one of them.
[capabilities]
sessions = "control" # none (default) | read | control
db = "kv" # none (default) | kv | tables
tasks = "write" # none | read | write (kernel-table access)
fs = ["{plugin_data}"] # path templates; empty by default
net = [] # allowed hosts; empty by default
shell = false # spawn processes
notify = true # post OS notifications

# A pane the plugin contributes to the layout. Visibility is kernel state:
# `default_visible` seeds it, the auto-generated `<plugin>.<pane>.toggle`
# command changes it, and it persists. See FEATURES-Keybindings.md §7.
[[panes]]
id = "tasks"
title = "Tasks"
slot = "right" # left | right | center | bottom | overlay
default_visible = true
min_width = 24
focusable = true
toggle_key = "f5" # optional default chord for the generated toggle command
documented_keys = [["space", "cycle status"]] # inert; shown in F1 as non-rebindable

# A command — callable by keybinding, palette, CLI, or agent.
[[commands]]
id = "tasks.create"
title = "New task"
description = "Create a task in the todo list"
args = { title = "string", description = "string?" }
agent_callable = true # default true; false hides it from agents

# A default keybinding. Users override in keybindings.json exactly as today.
[[keybindings]]
command = "tasks.create"
key = "n"
context = "pane:tasks" # global | pane:<id> | terminal

# Namespaced settings, surfaced in the Settings panel under the plugin's name.
[[settings]]
key = "show_done"
type = "bool"
default = true
label = "Show completed tasks"
```

**Every field is inert data.** The kernel reads the manifest without creating a
VM, so `thurbox plugin list`, the command palette, the keybinding editor, and
the Settings panel are all populated before a line of plugin code runs. A
plugin that never activates still contributes its keybindings — they simply
report the plugin as unavailable when invoked.

---

## 2b. Contribution points

Everything a plugin adds to thurbox is a **contribution**: declared in the
manifest, registered without creating a VM, and resolved by the kernel
([ADR-V15](ARCHITECTURE.md#adr-v15)). There is no imperative
`registerPane()` API.

| Contribution | Extends | Notes |
|---|---|---|
| `[[panes]]` | Slot-placed surfaces | `left` / `right` / `center` / `bottom` / `overlay` |
| `[[commands]]` | The command registry | Backs keys, palette, CLI, and agents alike |
| `[[keybindings]]` | Default chords | Land in `keybindings.json`, user-overridable as always. Contexts, conflict handling, and terminal passthrough: [FEATURES-Keybindings.md](FEATURES-Keybindings.md) |
| `[[settings]]` | Settings panel rows | Namespaced under the plugin |
| `[[agents]]` | `agents.toml` entries | Replaces the v1 extension capability |
| `[[automations]]` | Seeded schedules | Idempotent — matched by name |
| `[[statusItems]]` | Footer pills | `{ id, position, command }` |
| `[[tabs]]` | Central-pane tabs | For plugins contributing a `center` pane |
| `[[sessionDecorations]]` | Session-list rows owned by *another* plugin | See below |
| `[[cli]]` | A `thurbox-cli` verb namespace | Service half — [BACKEND-API §4](FEATURES-Backend-API.md#4-cli-verbs) |
| `[[skills]]` | Agent-facing markdown | [BACKEND-API §12](FEATURES-Backend-API.md#12-skills) |

Service-side registrations (`services`, `schedules`, `rpc`, `socket`, `spawn`)
are declared in code rather than the manifest, because they carry handlers;
their capabilities are still declared in the manifest so the kernel knows what
to grant before starting anything. See
[FEATURES-Backend-API.md](FEATURES-Backend-API.md).

### The remaining contribution shapes

`[[panes]]`, `[[commands]]`, `[[keybindings]]` and `[[settings]]` are in §2.
The other four are small enough to state in full:

```toml
# A footer pill. `command` runs on click; `text` is the static label the pill
# shows before the plugin is running, replaceable live via ctx.ui.setStatusItem.
[[statusItems]]
id = "ci-summary"
position = "right"        # left | right — ordered by declaration within a side
command = "ci.refresh"    # optional
text = "CI"               # optional static label

# A central-pane tab. Requires a pane with slot = "center".
[[tabs]]
pane = "ci-detail"
title = "CI"
shortcut = "f10"          # optional; same collision rules as any default chord

# An agents.toml entry. Fields are v1's AgentDef verbatim, so this is the
# extension capability moved, not redesigned.
[[agents]]
name = "ci-fixer"
command = "claude"
args = []
hook_schema = "claude"    # optional — see docs/AGENTS.md

# A seeded schedule, matched by name and idempotent across restarts.
[[automations]]
name = "ci-tick"
cron = "*/15 * * * *"
command = "ci.refresh"    # a command id, not a shell string
enabled = true
```

Three rules apply to all four. They are **inert data** like everything else in
the manifest, so they register with the plugin stopped. They are **idempotent**:
re-running discovery matches an existing `[[agents]]`/`[[automations]]` entry by
name rather than duplicating it, which is what replaces v1's self-heal loop
([MIGRATION §4](MIGRATION.md#4-teardown-inventory--the-v1-extension-system)).
And they are **removed on uninstall**, exactly as they were added.

### Cross-plugin composition

`sessionDecorations` is the primitive that lets plugins compose **without
knowing about each other**. A CI plugin declares:

```toml
[[sessionDecorations]]
id = "ci"
position = "trailing" # leading | trailing | subtitle
```

…and pushes decorations keyed by session id:

```lua
ctx.decorate("ci", { [sessionId] = { text = "✗ 2", tone = "danger" } })
```

The session-list plugin renders whatever decorations exist for a row without
importing, depending on, or knowing about the CI plugin. Without this, a
plugin that wants to annotate a session row must either fork the session-list
plugin or take a hard dependency on it — which is how plugin ecosystems
ossify.

### Activation events

A plugin's VM is created on its first activation event, not at launch:

| Event | Fires when |
|---|---|
| `onStartup` | TUI start — use sparingly; it is the only eager option |
| `onPaneVisible:<id>` | One of the plugin's panes becomes visible |
| `onCommand:<id>` | One of its commands is invoked (key, palette, CLI, agent) |
| `onSession` | Any session exists |
| `onEvent:<name>` | A kernel event it subscribes to fires |

```toml
activation = ["onPaneVisible:tasks", "onCommand:tasks.create"]
```

This is what keeps VM-per-plugin ([ADR-V4](ARCHITECTURE.md#adr-v4)) cheap
even at high N: N is bounded by plugins *in use*, not plugins *installed*, and a
plugin whose pane is never opened costs one manifest parse. A Luau state is
kilobytes and starts in microseconds, so this is an optimization rather than
the load-bearing constraint it was under a process-per-plugin model
([ADR-V2](ARCHITECTURE.md#adr-v2)).

---

## 3. Entry point

```lua
local thurbox = require("@thurbox")
local ui = thurbox.ui                    -- kernel primitives
local w = require("@thurbox/widgets")    -- userland widgets

type State = { tasks: { Task }, selected: number }

return thurbox.definePlugin({
    init = function(ctx): State
        return { tasks = ctx.tasks.list(), selected = 1 }
    end,

    render = function(ctx, state: State)
        return ui.box({ title = "Tasks", border = "focus" }, {
            w.list({
                id = "tasks",
                selected = state.selected,
                items = state.tasks,
                render = function(t)
                    return ui.row({}, { ui.text({ content = t.title }), statusGlyph(t.status) })
                end,
                onSelect = function(i) return { type = "select", index = i } end,
            }),
            w.keyHints({ hints = { { "n", "new" }, { "space", "cycle" }, { "d", "delete" } } }),
        })
    end,

    update = function(ctx, state: State, event)
        if event.type == "select" then
            return { tasks = state.tasks, selected = event.index }
        elseif event.type == "kernel:tasks-changed" then
            return { tasks = event.tasks, selected = state.selected }
        end
        return state
    end,

    commands = {
        ["tasks.create"] = function(ctx, args)
            local task = ctx.tasks.create({ title = args.title })
            ctx.dispatch({ type = "kernel:tasks-changed", tasks = ctx.tasks.list() })
            return { id = task.id }
        end,
    },

    dispose = function(ctx) end,
})
```

The shape is deliberately TEA, mirroring the kernel: `init` produces state,
`update` is a pure reducer over events, `render` is a pure function of state,
and side effects live in `commands` and in host API calls. `render` **must not
await, and must not call `ctx`** — it runs against in-memory state only, and its
callbacks *return* event descriptors rather than dispatching, because no
function crosses the wire
([FEATURES-View-Tree.md §1](FEATURES-View-Tree.md#1-model)).

Note what the entry point does **not** contain: the plugin's name, its panes,
its keybindings, or its settings. Those are manifest data, registered without
creating a VM at all ([ADR-V15](ARCHITECTURE.md#adr-v15)); the code
supplies only behavior. The `commands` map here provides *handlers* for command
ids the manifest declared — declaring a handler for an undeclared id is a load
error.

---

## 4. Lifecycle

```text
discover read plugin.toml; register panes/commands/keys/settings (no VM)
 │
 ▼
activate create Luau VM + thread → bind host tables → init() → first view
 │ (lazy: on first pane display or first command invocation)
 ▼
running kernel delivers events; plugin pushes views
 │
 ├── reload file change in dev mode, or `thurbox plugin reload <name>`
 │ → dispose() → drop the VM → new VM → init() → view;
 │ state is NOT preserved
 │
 ├── suspend all panes hidden for > 60 s → VM dropped, manifest stays
 │ registered; next display re-activates
 │
 └── fault error / deadline exceeded repeatedly
 → pane shows error state, backoff restart (1s, 2s, 4s, capped
 at 30 s), 5 consecutive faults → disabled; recover with
 `thurbox plugin reload <name>`, or `plugin enable <name>` after
 a `disable`. `plugin doctor` shows the last error.
 │
 ▼
deactivate dispose() with a 2 s grace period, then the VM's interrupt
 handler aborts the thread, then the VM is dropped
```

**Unload is a `Drop`, not a signal.** The VM is the unit of reload
([ADR-V2](ARCHITECTURE.md#adr-v2) C3): every value the plugin created —
closures, tables, host handles — lives inside that `Lua` state, so dropping it
reclaims all of them at once with no teardown protocol to get wrong. A plugin
that ignores its grace period is stopped by the VM-level interrupt handler,
which raises inside the interpreter loop and cannot be caught by plugin code.
This is only safe because plugins may not carry native code
([ADR-V2](ARCHITECTURE.md#adr-v2) C2) — a C module could hold a lock or a raw
pointer across the abort.

**Reload does not preserve state.** A plugin that wants durable state persists
it through `ctx.kv`. This keeps reload semantics honest — no partially
migrated in-memory shapes — and matches how `init()` runs on every activation.

---

## 5. The host boundary

There is no wire protocol. A plugin's VM is a `mlua::Lua` state owned by a
kernel thread ([ADR-V2](ARCHITECTURE.md#adr-v2)), so both directions of the
boundary are ordinary Rust ↔ Luau calls across it. The names below are the
*shape* of that boundary — the set of entry points and their semantics — not
message types on a socket.

### Kernel → plugin

The kernel calls into the VM from its own thread. Each call is bounded by the
VM's interrupt handler, so a plugin that spins is stopped rather than hanging
the kernel.

| Entry point | Kind | Carries |
|---|---|---|
| *(bind)* | at VM creation | host version, capabilities granted, theme tokens, pane rects |
| `init` | call → value | → initial view tree |
| `update` | call → value | input or kernel event (see §6) → new state |
| `commands[id]` | call → value | `(ctx, args)` → command result |
| `onSettingsChanged` | call | new values for the plugin's settings |
| `onThemeChanged` | call | new theme tokens |
| `onResize` | call | new rect for each of the plugin's panes |
| `dispose` | call | run teardown; the VM is dropped afterwards |

Capabilities are applied at bind time by **omitting** the tables a plugin was
not granted — an ungranted capability has no binding to reach
([ADR-V4](ARCHITECTURE.md#adr-v4)). There is no per-call permission check to
forget.

Note the shape of `update`: it is a **call the kernel makes**, not a request for
a frame. There is no `render` request — the kernel never asks a plugin for a
view, which is [ADR-V11](ARCHITECTURE.md#adr-v11)'s central rule and the reason
view pushes flow the other way.

Plugin `print` and uncaught error text are captured and written to
`thurbox.log`, tagged with the plugin name — the only place a plugin may write
free text (Constitution rule 9: stdout belongs to the TUI).

### Plugin → kernel

| Call | Kind | Carries |
|---|---|---|
| `ctx.push(tree)` | one-way | `{ paneId, revision, tree }` — the core render path |
| `ctx.surface.write(id, bytes)` | one-way | raw bytes into a `surface` grid, capability `pty` |
| `ctx.*` | yielding call | host API (§7) |
| `ctx.log(level, msg)` | one-way | a log line |

A view push is **one-way, not a return value**. The kernel never asks for a
frame; the plugin pushes when its state changes, and the kernel marks the UI
dirty ([ADR-V11](ARCHITECTURE.md#adr-v11)). Every frame paints from the last
pushed tree. Trees cross the boundary as an owned Rust value converted from the
Luau table once, at push time — the kernel never reads plugin memory during a
paint, so a reload mid-frame cannot tear the screen.

`surface.write` is the one path that carries per-frame data, and it is
deliberately *not* the view path: bytes go into a kernel-owned vt100 grid, not
into a tree ([ADR-V19](ARCHITECTURE.md#adr-v19)). It is subject to
backpressure — the kernel drops frames rather than queueing them, and reports
the drop count in `surface:stats`. A plugin that has no `surface` node never
uses it, which is the intended default.

### Deadlines and failure

| Situation | Kernel behavior |
|---|---|
| A command exceeds 250 ms | Status message "running…", result applied when it arrives |
| A command exceeds 10 s | Interrupt handler aborts the call; error surfaced in the status bar |
| No push after an event | Last tree keeps rendering — this is normal, not an error |
| An uncaught Luau error | Pane shows error state; backoff restart |
| Memory limit exceeded (`set_memory_limit`) | Allocation fails inside the VM; plugin faulted, VM dropped |
| A returned tree fails validation | Push rejected, plugin faulted, offending node logged |

---

## 6. Events

Delivered to `update(ctx, state, event)`.

**Input events** — only while one of the plugin's panes has focus:

```lua
{ type = "key", key = "n", mods = { "ctrl" } }
{ type = "paste", text = "…" }
{ type = "mouse", kind = "click", x = 12, y = 4, target = "row-7" }
{ type = "focus", focused = true }
```

`target` carries the id of the view-tree node under the cursor, so plugins do
not do hit testing — the kernel derives hitboxes from the tree.

One exception: while a `pty` or `surface` node holds focus it is an **input
sink** — keys are encoded to bytes and written to the grid rather than
delivered as `key` events. The plugin regains input when the node's `escape`
chord fires ([FEATURES-View-Tree.md §3.4](FEATURES-View-Tree.md#34-real-time-surfaces)).

**Kernel events** — delivered to any plugin that declared the matching
capability:

| Event | Requires |
|---|---|
| `session:created` / `:deleted` / `:statusChanged` / `:selected` | `sessions >= read` |
| `session:output` (debounced, no content) | `sessions >= read` |
| `task:changed` | `tasks >= read` |
| `git:worktreeChanged` | `sessions >= read` |
| `pty:exited` (`{ nodeId, code }`) | `pty` |
| `surface:stats` (`{ nodeId, dropped }`) | `pty` |
| `tick` (1 Hz) | none |

Events a plugin has no capability for are never delivered, so an unprivileged
pane sees only its own input.

---

## 7. Host API

Every call is a Rust function bound into the VM, gated by the manifest's
capabilities. Gating is by **absence**: a table the plugin was not granted is
never bound, so an ungranted call fails as `attempt to index nil` at the point
of use — an error, never a silent no-op, and one `luau-analyze` catches before
the plugin ever runs ([ADR-V4](ARCHITECTURE.md#adr-v4)).

```lua
export type HostContext = {
    -- Plain values, safe to read from `render`      no capability required
    pane: { id: string, width: number, height: number, focused: boolean },
    home: string,                        -- this plugin's directory
    settings: { [string]: any },         -- this plugin's settings values

    -- Rendering / state
    dispatch: (event: Event) -> (),      -- queue an event into own update()
    setState: (patch: { [string]: any }) -> (),
    invalidate: (paneId: string?) -> (),
    decorate: (id: string, bySessionId: { [string]: Decoration }) -> (),  -- §2b

    -- Sessions                                      capability: sessions
    sessions: {
        list: () -> { SessionInfo },                 -- read
        get: (id: string) -> SessionInfo,            -- read
        create: (cfg: SessionConfig) -> string,      -- control
        send: (id: string, text: string) -> (),      -- control
        focus: (id: string) -> (),                   -- control
        delete: (id: string, force: boolean?) -> (), -- control
    },

    -- Kernel tables                                 capability: tasks / automations
    tasks: TaskApi,
    automations: AutomationApi,
    messages: MessageApi,

    -- Plugin key-value storage                      capability: db >= kv
    -- Relational tables (ctx.db) are service-half only — BACKEND-API §7.2
    kv: {
        get: (key: string) -> any?,
        set: (key: string, value: any) -> (),
        delete: (key: string) -> (),
        list: (prefix: string?) -> { string },
    },

    git: GitApi,                         -- capability: sessions >= read

    -- UI affordances                                no capability required
    ui: {
        status: (message: string, level: ("info" | "warn" | "error")?) -> (),
        modal: (tree: Node) -> ModalResult,
        focusPane: (paneId: string) -> (),
        showPane: (paneId: string, visible: boolean) -> (),  -- a REQUEST
        setStatusItem: (id: string, item: StatusItem?) -> (),
        theme: () -> ThemeTokens,
    },

    -- Cross-plugin and cross-half channels
    bus: BusApi,                         -- capability: bus     BACKEND-API §8
    service: ServiceContract,            -- typed call into own service half

    notify: (title: string, body: string) -> (),      -- capability: notify
    exec: (cmd: { string }, opts: ExecOpts?) -> ExecResult,  -- capability: shell
    fetch: (url: string, init: FetchInit?) -> Response,      -- capability: net
    log: (level: string, message: string) -> (),
}
```

**Host calls look synchronous and are not blocking.** Every one of them yields
the plugin's coroutine and resumes when the kernel answers, so a plugin reads
like straight-line code while its thread stays available. `render` is the
exception — it may not call the host at all
([FEATURES-View-Tree §1](FEATURES-View-Tree.md#1-model)).

`ctx.pane`, `ctx.home`, and `ctx.settings` are plain values rather than calls,
which is what lets `render` branch on `ctx.pane.width`
([FEATURES-View-Tree.md §5](FEATURES-View-Tree.md#5-layout-algebra)) while still
obeying the no-host-calls-in-render rule. Everything else is async and belongs
in a command or an event handler.

The service half receives a different context — `ctx.db`, `ctx.sleep`,
`ctx.out`, `ctx.view`, and no `pane` or `ui` — specified in
[FEATURES-Backend-API.md](FEATURES-Backend-API.md). The two contexts share
`kv`, `bus`, `settings`, `home`, `log`, and the kernel-table APIs.

`fs` is a host API like the rest — `ctx.fs`, bound only when granted, and only
over the declared path templates. Luau ships no standard-library filesystem,
network or process access at all, so there is no ambient alternative for a
plugin to reach around it. Path templates expand at activation — `{repo}` (the
active session's primary repo), `{plugin_data}`
(`~/.local/share/thurbox/plugins/<name>/`), `{home}`.

> **All sixteen capabilities are enforced**, including `fs`, `net` and `shell`.
> Under the earlier sidecar design those three were advisory, because a
> JavaScript runtime without a permission model gave a plugin ambient access
> the manifest could not take away. Luau has no such ambient surface, so the
> path-template and host-allowlist arguments are checked inside the binding
> rather than merely printed at install time
> ([SECURITY.md §3](SECURITY.md#3-resolved--fs-net-and-shell-are-now-enforced)).

---

## 8. Capability reference

**This table is canonical.** It covers both halves;
[FEATURES-Backend-API.md §14](FEATURES-Backend-API.md#14-capabilities-used-by-this-half)
annotates the service-half subset rather than redefining it.

Grants are **per half** ([ADR-V16](ARCHITECTURE.md#adr-v16)). A flat
`[capabilities]` table grants both; `[capabilities.view]` and
`[capabilities.service]` narrow one of them, and win over the flat table where
they overlap. The **Half** column below is where a capability has any effect at
all — granting `pty` to a service or `cli` to a view is a manifest warning, not
a silent no-op.

| Capability | Values | Grants | Half |
|---|---|---|---|
| `sessions` | `none` \| `read` \| `control` | Session list/metadata; `control` adds create/send/focus/delete | both |
| `tasks` | `none` \| `read` \| `write` | Kernel task table | both |
| `automations` | `none` \| `read` \| `write` | Kernel automation table and manual runs | both |
| `messages` | `none` \| `read` \| `write` | The inter-session mailbox queue | both |
| `db` | `none` \| `kv` \| `tables` | `kv` = the plugin's own key space; `tables` = namespaced relational tables with kernel-executed migrations ([ADR-V17](ARCHITECTURE.md#adr-v17)) | both read `kv`; `tables` migrations are service-only |
| `fs` | path templates | Filesystem access, confined to the declared templates | both |
| `net` | host allowlist | Outbound HTTP; `["*"]` is permitted but flagged at install | both |
| `shell` | `bool` | Spawning processes | both |
| `notify` | `bool` | OS desktop notifications | both |
| `pty` | `bool` | `pty` / `surface` nodes — embedded programs and plugin-written grids | view |
| `cli` | `bool` | Registering `thurbox-cli` verbs | service |
| `services` | `bool` | Long-running supervised work — implies a resident VM | service |
| `schedules` | `bool` | Cron entries in the kernel scheduler | service |
| `bus` | `none` \| `own` \| `all` | `own` = publish on its own prefix; `all` = also subscribe to others' channels | both |
| `socket` | `bool` | Exposing control-socket methods | service |
| `spawn` | `none` \| `contribute` | Appending env/args at session spawn | service |

> **`pty` is not a weaker `shell`.** A `pty` node takes an arbitrary `command`,
> so `pty = true` is arbitrary code execution and the install prompt treats it
> as full trust. It differs from `shell` in exactly one way: the child's
> output goes into a kernel-owned grid the plugin cannot read back, so it is
> execution-capable but exfiltration-limited. Nothing else should be inferred
> from the split — it exists so a plugin that only *displays* a program does
> not also need `exec()`.

Defaults are the empty/`none` value for every capability: a plugin that
declares nothing can draw a pane, receive its own input, and nothing else.
`thurbox plugin install` prints the requested set and prompts on anything beyond
`db`; `--yes` skips the prompt for scripted installs.

Capabilities are **least privilege, not a security boundary**. The VM itself
is confined — no ambient I/O, bounded memory, interruptible — but a plugin
granted `shell`, or `fs` over a wide template, runs code under the user's own
account and is equivalent to full trust. Confinement bounds what an *unprivileged*
plugin can reach; it does not make a privileged one safe. They are an
informed-consent and least-privilege mechanism, and they are documented as such
at the install prompt.

Threat model, the enforceable/advisory split, and the changes required before
third-party plugins are installable: [SECURITY.md](SECURITY.md).

---

## 9. CLI surface

```bash
thurbox plugin list [--json] # installed, version, state, capabilities
thurbox plugin info <name> # manifest, panes, commands, keybindings
thurbox plugin install <name|url|path> # registry name, git URL, or local dir
thurbox plugin uninstall <name> [--purge] # --purge also drops plugin data
thurbox plugin enable|disable <name> # replaces v1's [features] flags
thurbox plugin update [<name>] [--all] # re-resolve from the recorded source (§10)
thurbox plugin reload <name> # respawn without restarting the TUI
thurbox plugin dev <path> # load from disk, watch, reload on save
thurbox plugin doctor # runtime, versions, faults, last errors
```

**Both binaries accept these.** `thurbox-cli plugin …` is the canonical form —
it is where v1 put every headless verb, it is what scripts and agents should
call, and it is what [FEATURES-Backend-API.md §4](FEATURES-Backend-API.md#4-cli-verbs)
reserves against plugin verb collisions. `thurbox plugin …` is an alias on the
TUI binary, because typing the CLI name to manage the thing you are looking at
is friction. Documentation uses the short form; anything scripted should use the
long one.

`plugin dev` is the development loop: it loads an unpacked directory, watches
the entry point's dependency graph, and reloads on save. Type errors surface in
the pane rather than the terminal, so the TUI never has to be restarted during
plugin development.

---

## 10. Versioning and compatibility

- The **host API** carries a major version, checked at bind time against the
 manifest's declared `api` major. A plugin built for an older major is refused
 with a clear message rather than half-working.
- The **manifest** declares `min_thurbox`; a newer requirement warns but does
 not block (mirroring v1's soft compat gate).
- **View-tree node types are additive.** Unknown node types render as a
 visible placeholder, never a crash — a newer plugin degrades on an older
 kernel instead of failing.
- **Command argument schemas are additive.** New optional arguments are
 allowed; removing or retyping one is a plugin major version.
- Bundled plugins version with the binary and are re-materialized whenever
 their embedded hash changes.
