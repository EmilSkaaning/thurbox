# Thurbox v2 — Architecture Decisions

Same mini-ADR format as [`../ARCHITECTURE.md`](../ARCHITECTURE.md):
**Choice**, **Why**, **Rejected alternatives**. Numbered `ADR-V*` so they never
collide with v1's `ADR-*` (architecture) or `ADR-P*` (performance) series.

## Index

| # | Decision | Load-bearing for |
|---|---|---|
| [V1](#adr-v1) | Kernel / plugin split — everything but six things is a plugin | The whole design |
| [V2](#adr-v2) | Plugins are Luau, in-process, one VM and thread each | Enforceable capabilities; footprint; reload |
| [V3](#adr-v3) | No bundled runtime — the VM is linked in | Install story; artifact size |
| [V4](#adr-v4) | One VM and thread per plugin, with declared capabilities | Enforcement by construction; blast radius; consent |
| [V5](#adr-v5) | Plugins return a declarative view tree; the kernel renders | Frame ownership; theming |
| [V6](#adr-v6) | Kernel-owned surfaces for content plugins cannot carry | Making "the session list is a plugin" affordable |
| [V7](#adr-v7) | Hierarchical TEA — plugins own their state | Constitution rule 7 |
| [V8](#adr-v8) | The v1 extension system is deleted, not migrated | One extensibility surface |
| [V9](#adr-v9) | Plugin storage is namespaced in the existing database | *Partly superseded by V17* |
| [V10](#adr-v10) | Every plugin command is agent-callable | The agent API |
| [V11](#adr-v11) | The frame budget is defended by protocol, not convention | Preserving ADR-P1–P12 |
| [V12](#adr-v12) | Module dependency rules extend to the kernel | Layer isolation |
| [V13](#adr-v13) | Keep ratatui as the kernel's rendering substrate | Not rewriting the render layer |
| [V14](#adr-v14) | Widgets are a Luau library; the catalog carries primitives | Keeping the catalog frozen |
| [V15](#adr-v15) | Contribution points with lazy activation | Registry without running plugins |
| [V16](#adr-v16) | A plugin has a headless service half and a TUI-only view half | Inheriting ADR-8b |
| [V17](#adr-v17) | Plugins declare migrations; the kernel executes them | Relational plugin data |
| [V18](#adr-v18) | Motion is declared, not pushed | Animation without a push per frame |
| [V19](#adr-v19) | Real-time and dense panes are vt100 surfaces | Live content without a second renderer |
| [V20](#adr-v20) | v2 is delivered on the trunk behind a compile-time gate | v1 stays shippable throughout |
| [V21](#adr-v21) | Pane visibility is kernel state; the F1 editor stays kernel | Preserving v1's panel toggles and rebinding |
| [V22](#adr-v22) | Anchored overlays instead of a floating-element ban | Dropdowns, context menus, inline compose |
| [V23](#adr-v23) | Pane geometry is a workspace tree; slots are a preset | Grids, spanning panes, nested splits |

---

<a id="adr-v1"></a>

## ADR-V1: Kernel / plugin split

**Choice**: Thurbox v2 is a **minimal Rust kernel** hosting **Luau plugins**
([ADR-V2](#adr-v2)). The kernel owns exactly six things:

| Kernel responsibility | Why it cannot be a plugin |
|---|---|
| Session lifecycle + backends (tmux, SSH, WSL, psmux) | Blocking process/IO supervision, control-mode protocol, restart/adopt correctness |
| Terminal grid (PTY read, vt100 parse, render) | Tens of KB of cells per frame per session — never crosses into a plugin VM |
| Storage (SQLite, schema, migrations, multi-instance sync) | Single writer, transactional integrity, `PRAGMA data_version` polling |
| Git + worktrees | Shared by every plugin; a plugin owning it makes it a hidden dependency |
| Event loop, layout solver, theme, keymap, frame compositor | Global invariants: focus is unique, panes cannot overlap, one frame per paint |
| Plugin host (VM lifecycle, host bindings, capabilities) | Bootstrap — it is what loads plugins |

Everything else — **including the session list** — is a plugin.

**Why**: The kernel list is precisely the set of things that are hard, shared,
and rarely modified. The plugin list is precisely the set of things that are
opinionated, frequently modified, and worth disagreeing about. v1 compiles both
into one 95k-line binary where changing the second requires understanding the
first.

**Rejected**:

- *Additive plugin API over the v1 core* — no first-party surface would
 dogfood the API, so it stays second-class and diverges from what built-ins
 can actually do. This is how most TUI plugin systems end up anemic.
- *Strangler migration keeping session list + terminal native* — cheaper, but
 leaves the two most important surfaces unable to demonstrate the API, and
 guarantees "why can't my plugin do what the session list does" forever.

---

<a id="adr-v2"></a>

## ADR-V2: Plugins are Luau, run in-process on a thread per plugin

**Choice**: A plugin is **Luau**, executed by a `Lua` VM embedded via
[`mlua`](https://github.com/mlua-rs/mlua) **inside the thurbox process**. Each
plugin gets its own VM on its own OS thread. The host surface is a set of Rust
functions bound into a sandboxed global environment; there is no RPC, no
serialization, and no child process.

Three conditions make this safe, and all three are binding:

| | Condition | Buys |
|---|---|---|
| **C1** | One VM on one thread per plugin; never the UI thread | A slow or wedged plugin cannot stall a frame ([ADR-V11](#adr-v11)) |
| **C2** | No native code — no FFI, no `dlopen`, no C modules | Crash isolation. This is the only thing a process boundary was buying that a thread cannot |
| **C3** | The VM is the unit of reload; the kernel holds no handle outliving it | Reload is a guaranteed clean slate, enforced by Rust lifetimes rather than discipline |

Full analysis in
[DECISION-plugin-runtime.md](DECISION-plugin-runtime.md).

**Why**: Three separate problems collapse into one solved one.

- **The capability model becomes real.** Luau is built to run untrusted code —
  it is Roblox's entire business — so there is no `io`, no `os.execute` and no
  `require` unless the host binds them. `fs`, `net` and `shell` stop being
  advisory and become enforced, closing the defect
  [SECURITY.md](SECURITY.md) had to leave open under every JavaScript option.
- **The bundled runtime disappears.** No 40–90 MB per platform archive, no
  per-channel vendoring across brew/AUR/Chocolatey/winget, no runtime CVE
  ownership, and no unanswered "does it build for musl and Windows ARM64".
  The VM is part of the binary.
- **Footprint and startup improve by orders of magnitude.** A Luau state is
  kilobytes and starts in microseconds, against tens of megabytes and tens of
  milliseconds per sidecar process. Lazy activation stops being load-bearing.

Two further properties made it acceptable rather than merely cheap. Luau has
**gradual typing** and `luau-analyze`, so the view-tree contract stays
statically checkable — the objection that rules out plain Lua. And Luau is a
fork of Lua 5.1, so ordinary Lua reads and runs largely unchanged: a plugin
author who has written a neovim config is not learning a new language.

**Rejected**:

- *TypeScript in a sidecar process over JSON-RPC* — what this ADR replaces,
  and the argument for it was sound: a process boundary contains crashes,
  hangs and native-code faults absolutely, reload is a guaranteed clean slate
  by construction, and npm is the largest library ecosystem there is. It was
  rejected on cost. Two halves across seven bundled plugins is plausibly
  200–400 MB of resident runtime for a terminal tool, and the boundary buys
  its isolation by paying for a JS runtime thurbox then has to vendor, patch
  and ship on four platforms — while *still* not being able to enforce
  `fs`/`net`/`shell`, because no shipping JS runtime except Deno has a
  permission model. C1–C3 recover every isolation property except protection
  from native code, and C2 removes native code.
- *Plain Lua 5.4* — the standard, so documentation, snippets, every neovim
  user and every language model know it better. Rejected because it has no
  gradual typing (Teal is another toolchain) and its sandbox is one you
  assemble and maintain yourself rather than one an upstream maintains because
  its business depends on it. The familiarity gap is narrow since Luau runs
  ordinary Lua; the sandbox and typing gaps are not.
- *LuaJIT* — fastest, frozen at 5.1 semantics with upstream in maintenance,
  and its built-in `ffi` is exactly what C2 forbids, so it ships with the
  sharp edge enabled. JIT also means writable-executable pages, which some
  hardened environments refuse.
- *QuickJS (`rquickjs`)* — would keep TypeScript in-process and satisfies
  C1–C3, but gives up the sandbox that is Luau's main prize while keeping a
  language that is no longer a requirement.
- *Rhai* — the only candidate written in pure Rust, which removes even the
  residual risk of the VM itself faulting, with sandboxing and operation
  limits first-class. Rejected on adoption: a plugin system exists to attract
  third-party authors, and almost nobody writes Rhai.
- *Starlark* — hermetic by construction, and a *configuration* language: no
  I/O, restricted recursion and loops. Right for `layout.toml`-shaped
  problems, wrong for something that holds state and reacts to events.
- *Embedded V8 (`deno_core`)* — keeps TypeScript and npm, and drags a large
  V8-adjacent tree through `cargo-deny` for the life of the project while its
  per-isolate memory gives back the reason for going in-process at all.
- *WASM components (`wasmtime` + WIT)* — the strongest sandbox and
  language-agnostic, but every host API must be hand-bound in WIT, the guest
  toolchain for a scripting-shaped language is poor, and the dev loop regains
  a build step that C3's reload story exists to remove.
- *Rust dynamic libraries* — fastest, and reload requires recompiling, which
  contradicts the premise. Rust has no stable ABI, so every kernel release
  would break every plugin binary.

**Reversal conditions**, since this is a decision taken on analysis rather
than measurement: plugins routinely needing third-party libraries Luau cannot
supply, or agent-authored plugins proving materially worse in Luau than in a
mainstream language. Both are tested at the validation gate in
[DECISION-plugin-runtime.md](DECISION-plugin-runtime.md).

---

<a id="adr-v3"></a>

## ADR-V3: No bundled runtime — the VM is linked into the binary

**Choice**: thurbox ships **one binary with no runtime dependency**. The Luau
VM is compiled in via `mlua` ([ADR-V2](#adr-v2)). There is nothing to vendor,
nothing to detect on the user's machine, and no `THURBOX_PLUGIN_RUNTIME`
override.

**This ADR previously said the opposite** — it specified bundling a pinned Bun
per platform archive, accepting 40–90 MB of artifact growth as the price of
plugins working on a machine with no Node ecosystem. Moving in-process
([ADR-V2](#adr-v2)) does not reduce that cost; it deletes it.

**Why**: The v1 install story survives untouched. `brew install thurbox &&
thurbox` still works on a bare machine, `install.sh` still fetches one
artifact, and the four platform builds stay the size they are. The obligations
the bundled-runtime design created — per-channel packaging work across
brew/AUR/Chocolatey/winget, ownership of a third-party runtime's CVE response,
and an unanswered question about musl and Windows ARM64 — do not arise.

**What this closes**:

| Was open | Now |
|---|---|
| 40–90 MB per platform archive | No change from v1 |
| Runtime CVE response, shippable as a patch release | `mlua` is an ordinary Cargo dependency under `cargo-deny` |
| Does a suitable runtime exist for musl / Windows ARM64 / macOS universal? | Moot — it compiles wherever the binary does |
| Packaging churn on four moderated channels | None |

**Rejected**:

- *Bundling a JS runtime* — the previous choice, and only necessary because
  plugins ran out of process in a language thurbox does not compile.
- *Requiring a system Lua* — smaller still, and reintroduces exactly the
  "works on my machine" surface that pinning was meant to remove, for a
  dependency measured in kilobytes.

---

<a id="adr-v4"></a>

## ADR-V4: One VM and one thread per plugin, with declared capabilities

**Choice**: Each plugin gets **its own Luau VM on its own OS thread**. Its
manifest declares the capabilities it needs; the kernel binds **only** the host
functions those capabilities grant into that VM's sandboxed global environment.

```toml
[capabilities]
sessions = "read"                  # none | read | control
db       = "kv"                    # none | kv | tables
fs       = ["{repo}", "{plugin_data}"]
net      = ["api.github.com"]
shell    = false
```

**Why**:

- **Enforcement is by construction, not by checking.** A plugin without `fs`
  does not fail an `fs` call — there is no `io` table in its environment to
  call. This is the difference between a capability system and a convention,
  and it is what [ADR-V2](#adr-v2) buys.
- **Blast radius.** A plugin that throws returns a `Result`; one that loops is
  aborted by Luau's interrupt handler; one that allocates without bound hits
  its per-VM memory limit. Each affects one pane.
- **Reload granularity.** Dropping one VM does not disturb the others, and
  costs microseconds rather than a process respawn.
- **Informed consent.** `thurbox plugin install` shows exactly what a plugin
  asked for, and a pane that only draws rows can be denied everything. v1
  extensions ran arbitrary shell with no declaration; plugins run
  continuously, so trust-on-install alone was a downgrade.

**The limit, stated plainly**: C2 ([ADR-V2](#adr-v2)) forbids native code
because a segfault in a plugin's C module would take the TUI with it — the one
isolation property a thread cannot recover. Capabilities constrain what a
plugin may *call*; C2 is what stops it reaching around them.

**Rejected**:

- *One process per plugin* — absolute crash isolation including native faults,
  at 15–30 MB resident each and a JS runtime to vendor. See
  [ADR-V2](#adr-v2)'s rejected list.
- *One VM shared by all plugins* — lowest memory, and one plugin's globals,
  faults and reloads become everyone's. Sandboxing between plugins inside a
  single Lua state is far harder than between states.
- *Capability checks at each host function instead of environment binding* —
  equivalent on paper, worse in practice: it puts a check in every binding
  rather than a decision in one place, and a missed check is a silent hole.

---

<a id="adr-v5"></a>

## ADR-V5: Plugins return a declarative view tree; the kernel renders it

**Choice**: A plugin never draws. It returns a **declarative view tree** — a
plain-data node graph of `box`, `list`, `text`, `markdown`, `input`, `table`,
… , carrying no closures and no host handles, converted once to an owned Rust
value at push time —
and the kernel renders it with ratatui using the active theme, derives mouse
hitboxes from it, and routes input back as events. Full specification in
[FEATURES-View-Tree.md](FEATURES-View-Tree.md).

**Why**:

- **The frame stays the kernel's.** Focus uniqueness, pane bounds, theme
 consistency, scrollbars, and the demand-driven redraw loop (ADR-P*) remain
 enforceable invariants rather than plugin conventions.
- **Plugins cannot break rendering.** The worst a plugin can do is return an
 ugly tree or none at all. It cannot corrupt the terminal, leak escape
 sequences, or desynchronize the cursor.
- **Theming and accessibility come free.** Nodes reference semantic theme
 tokens, so all 36 palettes and every future one apply to third-party panes
 with no plugin change.
- **It is one value, not a call stream.** An immediate-mode drawing API would
 mean hundreds of host calls per frame and a plugin holding the frame buffer;
 a tree is one value handed over once.

**Rejected**:

- *Immediate-mode drawing into a rect* — maximum power, but per-frame call
 chatter, off-theme output, and the ADR-P performance work becomes
 unholdable.
- *Data-only slots (plugin supplies rows, kernel owns the widget)* — simplest
 and most consistent, but no plugin could ever build something like the
 code-review diff, and ADR-V1 requires that built-ins be plugins.
- *Declarative tree plus a raw-buffer escape hatch* — the pragmatic middle,
 and reconsidered if a real pane proves unbuildable. Deferred because two
 rendering paths means two sets of failure modes, and the escape hatch always
 becomes the path of least resistance.

---

<a id="adr-v6"></a>

## ADR-V6: Kernel-owned surfaces for content plugins cannot carry

**Choice**: The view tree includes a small set of **surface nodes** that a
plugin *places and configures* but does not *supply content for*. The kernel
renders them directly from kernel-owned state:

| Node | Kernel state rendered |
|---|---|
| `sessionTerminal { sessionId, scroll }` | The live vt100 grid for that session |
| `pty { command, args, env, cwd, scroll }` | A live PTY process the kernel spawned |
| `surface { id, cols, rows }` | A vt100 grid the plugin writes bytes into |
| `diff { repo, target }` | A git diff the kernel computed |
| `sparkline { metric }` | Kernel metrics ring buffers |

**Why**: A session's terminal grid is ~200 rows × 200 columns of styled cells,
changing many times a second. Rebuilding that as a tree in a plugin and
converting it back every frame would cost more than the entire v1 render loop. The plugin decides
*where* the terminal goes, how big it is, and what surrounds it; the kernel
fills the rect. This is what makes "the session list is a plugin" affordable
rather than aspirational.

`pty` and `surface` extend the same mechanism past sessions: the first to an
arbitrary process, the second to a grid with no process at all. Both reuse the
`vt100` + `tui-term` pipeline the session pane already runs on, so neither adds
a dependency or a second renderer. Their contract — input, resize, lifetime,
and why they are the answer to real-time content rather than a loophole in
[ADR-V5](#adr-v5) — is [ADR-V19](#adr-v19).

**Rejected**:

- *Stream the grid to plugins* — conceptually pure, catastrophic in practice.
- *Keep the terminal as a hardcoded pane outside the tree* — then plugins
 cannot lay out around it, and the central pane's tab strip (agent / shell /
 review) cannot be a plugin.

---

<a id="adr-v7"></a>

## ADR-V7: Plugins own their state; the kernel owns kernel state

**Choice**: TEA is preserved but becomes **hierarchical**. The kernel keeps a
single model for kernel state (sessions, focus, layout, theme). Each plugin
keeps its own model, updated by its own reducer, in its own VM. The
kernel treats a plugin as a pure function of `(events) → view tree`; it never
inspects plugin state.

**Why**: Constitution rule 7 forbids component-local state because in v1 it
would mean state scattered across panes that all mutate the same `App`. That
hazard does not exist across a VM boundary: a plugin's state cannot be
reached, aliased, or corrupted by anything but its own reducer. The
one-directional `Event → Message → update → view → Frame` flow holds at both
levels; there are simply two levels. Constitution rule 7 is amended
accordingly — see [CONSTITUTION-DELTA.md](CONSTITUTION-DELTA.md).

**Rejected**:

- *Single global model including plugin state* — every plugin state change
 becomes a host call plus a broadcast; enormous chatter for no invariant that
 ADR-V5 does not already give us.
- *Free-form plugin state with callbacks* — the thing rule 7 was written to
 prevent, and untestable.

---

<a id="adr-v8"></a>

## ADR-V8: The v1 extension system is deleted, not migrated

**Choice**: `extensions/`, `ExtensionDef`, `extension_config`,
`session_ops::extensions`, `builtin_hooks`, the `thurbox-cli extension`
subcommand, and the `active_extensions` metadata key are **removed** in v2.
Plugins are the only extensibility surface. No compatibility shim, no
deprecation window.

**Why**: The extension system is unused in practice, and keeping it would mean
shipping two manifest formats, two installers, two update paths, two self-heal
loops, and two answers to "is tasks an extension or a plugin?" for the rest of
the project's life. Its genuinely useful capabilities — declaring agents,
seeding automations, placing files into agent config dirs, patching agent args
— are re-expressed as plugin capabilities, which is strictly more general
because a plugin can also draw.

**What must survive the deletion**: the built-in **hooks** behavior. Agent
status reporting (`working`/`blocked`/`done`) is core product behavior, not an
extension, and is absorbed into the kernel's session layer. See
[MIGRATION.md](MIGRATION.md) for the full teardown inventory.

**Rejected**:

- *Unify by re-expressing extensions as plugins* — the right answer if they
 had users. They do not.
- *Ship the extension machinery as one bundled plugin* — preserves v1 behavior
 verbatim, but keeps two user-facing concepts alive indefinitely to serve
 nobody.

---

<a id="adr-v9"></a>

## ADR-V9: Plugin storage is namespaced inside the existing database

> **Partly superseded by [ADR-V17](#adr-v17).** The "plugins do not create
> tables" clause was too strong and no longer holds: plugins may declare
> migrations that the *kernel* executes into a per-plugin namespace. Everything
> else below stands — one database file, one schema owner, coherent
> multi-instance sync, no raw SQL handle, kernel tables reachable only through
> typed host APIs. Read this ADR for **why storage lives in the kernel
> database**; read ADR-V17 for **what a plugin may do with it**.

**Choice**: Plugin storage lives in the existing SQLite database: a namespaced
key-value store (`plugin_kv(plugin, key, value, updated_at)`) plus read access
to kernel tables through typed host APIs. No plugin receives a raw SQL handle.

**Why**: One database keeps the multi-instance sync story (`PRAGMA
data_version`, ADR-7b) intact for free, keeps backup/restore a single file,
and keeps schema migrations under one owner. A plugin that could `CREATE
TABLE` would make `SCHEMA_VERSION` meaningless and turn every plugin
uninstall into an orphaned-table problem.

**Consequence**: The v1 `tasks`, `automations`, `session_messages`,
`review_comments`, and `review_marks` tables stay kernel tables with typed
host APIs, because bundled plugins need them and because the headless CLI and
the automation heartbeat use them without any plugin loaded. "Tasks is a
plugin" means the *pane and its behavior* are a plugin, not that the storage
moves.

**Rejected**:

- *Per-plugin SQLite file* — clean isolation, but breaks cross-plugin joins,
 multiplies the sync mechanism, and scatters backups.
- *Raw SQL handle scoped by capability* — maximum power, no way to enforce
 schema ownership, and a single bad migration corrupts the user's sessions.

---

<a id="adr-v10"></a>

## ADR-V10: Every plugin command is agent-callable

**Choice**: Plugins register **named commands with JSON-Schema-typed
arguments**. The same registry backs keybindings, the command palette,
`thurbox-cli`, and a JSON-RPC control socket. Anything a user can trigger, an
agent can trigger.

**Why**: Thurbox already puts an agent inside every session, and v1's flow and
shepherd workflows already coordinate through `thurbox-cli` and the message
queue. Making the plugin surface agent-callable is the natural completion of
that: a session's agent can create a task, open a review, or drive a pane
without screen-scraping. It also gives plugins one entry-point abstraction
instead of three parallel ones. Full design in [FEATURES-Agent-API.md](FEATURES-Agent-API.md).

**Rejected**:

- *Read/query only* — safer, but reduces agents to observers when the whole
 point of thurbox is agents doing work.
- *Defer to v2.1* — the command registry is load-bearing for keybindings and
 the palette regardless; retrofitting the agent surface later means
 redesigning schemas after third-party plugins depend on them.

---

<a id="adr-v11"></a>

## ADR-V11: The frame budget is defended by protocol, not by convention

**Choice**: The render protocol is designed so a slow plugin degrades its own
pane and nothing else:

1. **Views are cached and versioned.** The kernel keeps each plugin's last
 view tree. A plugin pushes a new tree when its state changes; the kernel
 does not request one per frame.
2. **Rendering never blocks on a plugin.** A frame always paints from cache. A
 plugin that has not answered simply shows its previous tree.
3. **Every call into a plugin has a deadline** (default 250 ms), enforced by
 the VM's interrupt handler. On timeout the pane renders its last tree with a
 staleness marker; repeated timeouts suspend the plugin with a visible error
 state.
4. **The demand-driven loop is preserved.** A plugin pushing a view marks the
 UI dirty exactly as `detect_output_redraw` does today. Idle plugins cost
 zero frames.
5. **Input is acknowledged, not awaited.** Key events are dispatched
 fire-and-forget; the resulting view arrives when it arrives.

**Why**: The v1 performance work (`docs/PERFORMANCE.md`, ADR-P1–P12) is a real
asset — idle paints dropped from ~100 fps to ~4 fps, the whole new-session
flow moved off the UI thread. A naive request-response render protocol would
put a plugin call on the critical path of every frame and undo all of it.

**Rejected**:

- *The kernel calls `render()` on every frame* — the obvious design, and the
 one that makes the TUI exactly as fast as its slowest plugin. In-process this
 is *more* tempting than it was over a socket, and no less wrong: the render
 thread would take a lock on every plugin's VM before it could paint.
- *Plugins draw into a shared cell buffer* — removes the tree conversion but
 reintroduces ADR-V5's rejected immediate mode plus a memory-safety problem
 that [C2](#adr-v2)'s no-native-code rule exists to avoid.

---

<a id="adr-v12"></a>

## ADR-V12: Module dependency rules extend to the kernel

**Choice**: The `tests/architecture_rules.rs` allowlist continues to govern
the Rust kernel, with one new module family:

```text
session ← pure data types, no crate-internal references
agent ← session
plugin ← session (+ paths/shell); NEVER ui, git, app
ui ← session + app model/view state; NEVER agent, git, plugin
app ← coordinator, imports all modules
```

`plugin` (host, supervisor, host bindings, capabilities) sits beside `agent` as a
side-effect layer: it owns VM lifecycles and plugin threads, so it may not be
imported by `ui`. The view-tree renderer lives in `ui` and consumes plain
data types from `session`, exactly as the diff types do today (v1 ADR
precedent: `session::review` exists so `ui` can render diffs without importing
`git`).

**Why**: The isolation rule is the reason v1's layers stayed clean under
95k lines. A new subsystem owning foreign VMs and threads is precisely the
kind of thing it exists to fence off.

**Rejected**:

- *Let `ui` call the plugin host directly to fetch views* — convenient, and it
 would make `view()` impure and blocking, violating both the TEA rule and
 ADR-V11.

---

<a id="adr-v13"></a>

## ADR-V13: Keep ratatui as the kernel's rendering substrate

**Choice**: The kernel keeps **ratatui + crossterm + tui-term + vt100**. The
view-tree renderer is written as a ratatui consumer: nodes resolve to
`Rect`s through ratatui's constraint solver and paint as `Line`/`Span` runs
into the frame buffer.

**Why**: Auditing how v1 actually uses ratatui settles this more firmly than
expected.

| Signal | Measurement |
|---|---|
| ratatui widget types used | 5 (`Block`, `Borders`, `Clear`, `ListItem`, `Paragraph`) |
| `Span::styled` / `Line::from` call sites | 395 / 168 |
| Files touching the raw cell buffer | 2 (`selection.rs`, `terminal_view.rs`) |
| Pure view-model structs handed to renderers | 19 (`TaskPaneState`, `LeftPanelState`, …) |

Two conclusions follow:

1. **thurbox uses ratatui as a substrate, not a widget toolkit.** What it
 consumes is the cell buffer with double-buffered diffing (only changed
 cells are written — the thing that makes the demand-driven loop in
 `docs/PERFORMANCE.md` affordable), the constraint layout solver, the
 span/style model, and crossterm integration. It does not consume ratatui's
 widget library, so nothing about the widget library constrains v2.
2. **v1 already has the view-tree architecture — statically.** Each pane
 renderer is a pure function from a plain view-model struct to spans, built
 by the `app` layer. That is a view tree with 19 hand-written node types and
 19 bespoke renderers. v2 replaces those with one node set and one renderer.
 This is a refactor of an existing pattern, not the introduction of a new
 one.

`tui-term` + `vt100` are separately decisive: they bridge a tmux-driven vt100
grid into the frame, which is what makes the agent panes work at all, and they
have no equivalent outside the Rust ecosystem.

**Rejected**:

- *Move the whole TUI to TypeScript* — the serious alternative. Rust demotes
 to a headless session daemon; the TUI is a TS app on
 [OpenTUI](https://github.com/anomalyco/opentui) (React/Solid components over
 a Zig rendering core via Bun FFI, used in production by opencode — a CLI
 thurbox itself launches) or pi's `pi-tui`. Plugins would then be in-process
 TS modules: no serialization, no node catalog, unlimited widget power, one
 language. Rejected because (a) the central pane is N live vt100 grids driven
 by tmux control mode, so this means re-solving `vt100` + `tui-term` in
 JavaScript on the hottest path thurbox has; (b) the kernel stays Rust
 regardless — tmux control mode, SSH/WSL transports, SQLite, git — so the
 outcome is still two languages, with the process boundary moved from a quiet
 place (view trees, pushed on change) to a chatty one (terminal cells, every
 frame); (c) it is a rewrite of the event loop, input translation, selection,
 links, and mouse routing, not a refactor; (d) OpenTUI is pre-1.0, and
 betting the entire render layer on it is a categorically larger risk than
 betting the plugin layer on a runtime. Named here because if the view tree
 ever proves too constraining, this is the direction to reconsider — not a
 raw-buffer escape hatch bolted onto ADR-V5.
- *A different Rust TUI crate* (`cursive`, `iocraft`, `zi`) — each would still
 need the same view-tree layer built on top, none has the `tui-term`/`vt100`
 bridge, and all would cost a full port for no capability gain.
- *Render directly on crossterm* — drops a dependency and rebuilds buffer
 diffing plus a layout solver by hand, which is the part of ratatui thurbox
 actually depends on.

---

<a id="adr-v14"></a>

## ADR-V14: Widgets are a Luau library; the node catalog carries primitives

**Choice**: The view-tree node catalog is **two tiers**, and only these cross
into the kernel as node types:

- **Tier 1 — primitives.** `box`, `row`, `column`, `spacer`, `scroll`, `text`,
  `input`, `textarea`. Deliberately small and **frozen**.
- **Tier 2 — kernel surfaces.** Nodes whose content or clock lives in the
  kernel: `sessionTerminal`, `pty`, `surface`, `diff`, `fileTree`, `code`,
  `markdown`, `statusDot`, `sparkline` ([ADR-V6](#adr-v6)).

Everything else — tables, badges, progress bars, key-hint rows, empty states,
selectable lists — is **`thurbox.widgets`, an ordinary Luau module** that
composes down to Tier 1.

The line has one test: **does the kernel own the data or the clock?**

**Why**: A featureful kernel catalog makes the kernel the bottleneck for every
new widget. If an author who needs a tree-table has to file an issue and wait
for a thurbox release, the API is decorative. Keeping widgets in plugin-space
means they are written by whoever needs them, testable without a kernel, and
forkable when opinions differ — and it keeps the protocol small enough that
the additive-only rule
([N4](CONSTITUTION-DELTA.md)) is holdable.

**The distribution cost, stated because it is real.** An earlier draft of this
ADR put widgets on npm, where versioning and forking are solved. Luau has no
npm ([ADR-V2](#adr-v2)), so `thurbox.widgets` ships as a bundled module
resolvable by every plugin, with third-party widget libraries distributed as
ordinary plugins that export modules. That pulls widget versioning **back
toward the kernel's release cycle**, which is precisely the coupling this ADR
exists to prevent. It is the sharpest cost of the Luau decision and it is not
fully bought off: the mitigation is that a plugin may vendor its own widget
module and ignore the bundled one, so the bundled version is a default rather
than a mandate.

**Rejected**:

- *Rich kernel catalog* — better defaults and less plugin code, at the cost of
  making the kernel the bottleneck for every widget forever.
- *Pure primitives, no Tier 2* — conceptually clean and unable to render a
  terminal grid or a diff, which is the whole point of ADR-V6.
- *Widgets as a kernel-versioned standard library with no override* — simpler
  to document, and it makes the coupling above permanent instead of a default.

---

<a id="adr-v15"></a>

## ADR-V15: Contribution points with lazy activation

**Choice**: A plugin extends thurbox by declaring **contributions** in its
manifest — never by imperative registration at runtime. Each contribution
type is registered from the manifest **without starting the plugin's VM**;
the VM is created only when an **activation event** fires.

| Contribution | Extends |
|---|---|
| `panes` | Slot-placed surfaces |
| `commands` | The command registry (keys, palette, CLI, agents) |
| `keybindings` | Default chords, user-overridable as usual |
| `settings` | Namespaced rows in the Settings panel |
| `agents` | Entries in `agents.toml` |
| `automations` | Seeded schedules |
| `statusItems` | Footer pills |
| `tabs` | Central-pane tabs |
| `sessionDecorations` | Badges/columns on session-list rows owned by another plugin |

Activation events: `onStartup`, `onPaneVisible:<id>`, `onCommand:<id>`,
`onSession`, `onEvent:<name>`.

**Why**:

- **Nothing runs that nobody asked for.** Under
 [ADR-V2](#adr-v2) a VM costs kilobytes and microseconds, so this is no
 longer load-bearing for startup the way process-per-plugin made it — but a
 plugin that never activates should still execute no code, which is a
 correctness and consent property rather than a performance one.
- **The UI is complete before any plugin runs.** Keybindings resolve, the
 palette lists commands, Settings shows every row, and panes reserve their
 slots — all from manifest data. A plugin that never activates costs nothing
 and still appears in the F1 editor.
- **`sessionDecorations` is how plugins compose without knowing about each
 other.** A CI plugin adds a failing badge to a session row owned by the
 session-list plugin, with no dependency between them. Without a decoration
 point, cross-plugin composition forces plugins to import each other, which
 is how plugin ecosystems ossify.

**Rejected**:

- *Imperative registration in `init()`* (the Ink/Express shape) — more
 flexible, and it requires running every plugin at startup to know what the
 keybindings even are.
- *Eager activation of everything* — simpler lifecycle, N cold starts on every
 launch, and idle memory proportional to plugins installed rather than
 plugins used.

---

<a id="adr-v16"></a>

## ADR-V16: A plugin has a headless service half and a TUI-only view half

**Choice**: A plugin declares up to two entry points — `service.luau` and
`view.luau` — running in **separate VMs on separate threads** with separate
capability grants.
The service half is hosted by whichever of the TUI, the tmux heartbeat keeper,
or a `thurbox-cli` invocation needs it first; the view half exists only while
the TUI does. Full contract in [FEATURES-Backend-API.md](FEATURES-Backend-API.md).

**Why**: v1 guarantees automations fire with the TUI closed (`docs/ARCHITECTURE.md`
ADR-8b — a detached keeper loops `automation tick` every 60 s, with optional
systemd/launchd units). A plugin backend that only lived inside the running TUI
would silently revoke that: a task-sync or CI-watch plugin would stop the
moment you quit. Drawing the seam here means the plugin system inherits ADR-8b
rather than fighting it.

Three further properties fall out:

- **A plugin with no UI is just a service.** The tracker-sync integrations that
 v1 shipped as `Exec` automations become first-class plugins with no view
 tree at all.
- **Capabilities split naturally.** A view rarely needs `net` or `shell`; a
 service rarely needs anything else. Granting per half makes the install
 prompt meaningful instead of a union of everything.
- **The halves fault independently.** A broken sync loop degrades a pane; it
 does not remove it.

Machine-wide single-instance is enforced with an advisory lock in the database,
so a running TUI and a heartbeat tick never both run a plugin's poll loop.

**Rejected**:

- *One process per plugin doing both jobs* — simpler, and it either dies with
 the TUI (losing ADR-8b) or keeps a renderer resident headless (paying for a
 view nobody can see).
- *Service work as kernel automations only* (`Exec` shelling out, the v1
 pattern) — no state, no typed contract, no supervision, and every
 integration reduced to parsing its own output.
- *Service half in Rust* — faster, and it reintroduces recompiling for the
 majority of what an integration plugin actually does.

---

<a id="adr-v17"></a>

## ADR-V17: Plugins declare migrations; the kernel executes them

**Choice**: A plugin may own **relational tables** in the kernel database. It
declares an append-only migration list; the kernel rewrites each statement into
the plugin's namespace (`runs` → `plugin_ci_runs`), executes it, and records
the applied version in `plugin_migrations(plugin, version)`. Queries go through
parameterized `ctx.db.all/run/tx`. There is no raw connection handle, and a
statement touching anything outside the namespace is rejected.

**Why**: [ADR-V9](#adr-v9)'s KV-only store is genuinely painful for a plugin
with relational data — a CI plugin tracking runs across repos and branches ends
up hand-rolling indexes in a key space. The usual answer in JS plugin ecosystems
is to hand the plugin a live `better-sqlite3` handle, and that is undeniably
more useful. But a raw handle would make `SCHEMA_VERSION` meaningless, let one
plugin read another's data (and the user's sessions), and turn every uninstall
into an orphaned-table problem.

Kernel-executed migrations keep the useful half and drop the dangerous half:
one schema owner, coherent `PRAGMA data_version` sync (ADR-7b), namespace
isolation enforced rather than promised, and `plugin uninstall --purge` that
actually removes everything.

**Rejected**:

- *A raw `Database` handle* — maximum power, and it forfeits schema ownership,
 isolation, and clean uninstall in one step.
- *A separate SQLite file per plugin* — clean isolation, and it multiplies the
 sync mechanism, breaks single-file backup, and still needs migration
 machinery.
- *Staying KV-only* — the least code, and it pushes every plugin with real
 data into either a hand-rolled index or its own file outside our control,
 which is worse than granting the namespace.

---

<a id="adr-v18"></a>

## ADR-V18: Motion is declared, not pushed

**Choice**: An animation is a **property of a node**, evaluated by the kernel
on its own frame clock, not a sequence of view pushes. A plugin declares what
should move and how; it pushes once.

```lua
ui.text({ content = label, motion = { kind = "marquee", cps = 6 } })
ui.box({ id = "thinking", motion = { kind = "cycle", fps = 8, frames = THINKING } })
```

The kernel grants the pane an **animation lease** while a live `motion` is in
its tree: that pane — and only that pane — is exempt from the 250 ms redraw
floor, up to a declared, capped rate. Leases drop when the pane is hidden,
when it is unfocused and the motion declared `pauseWhenUnfocused`, and when
the plugin's next push contains no motion. Catalog in
[FEATURES-View-Tree.md §3.3](FEATURES-View-Tree.md#33-motion); the normative
spec — phase and restart semantics, lease budgeting, accessibility caps,
determinism — is [FEATURES-Animation.md](FEATURES-Animation.md).

**Why**: The cost of animation in a push model is not the animation. It is the
four things a push drags with it — a call into the plugin, a tree rebuild, a
conversion and diff, and a paint — of which only the paint is inherent. Moving the clock into
the kernel deletes the other three and leaves a cost identical to what
`statusDot` already pays today.

This is also the honest reading of [ADR-V14](#adr-v14)'s tier test. "Does the
kernel own the data or the clock?" is two questions, and `statusDot` was
sitting at an answer nobody had generalized: **the plugin owns the data, the
kernel owns the clock**. Every cosmetic animation in
[LIMITATIONS §2.2](LIMITATIONS.md#22-animation) — a spinner of your own
design, a pulsing progress bar, a typing indicator, marquee text — is that
same split. Tier 1 does not grow; a field on the envelope does.

Three properties make it safe to grant:

- **Advisory.** A kernel that declines to animate — a `reduce_motion` setting,
 a pane over its rate budget — renders frame 0. Nothing breaks, and
 accessibility gets a switch for free.
- **Bounded by construction.** Frames are supplied up front, so a motion's
 cost is known at push time rather than discovered at runtime. Rate is capped
 (30 fps per pane, 30 fps aggregate across panes, degraded round-robin).
- **Visible.** Live leases are listed by `thurbox plugin doctor` and counted
 in the perf HUD, so "why is thurbox waking up" always has an answer.

**Rejected**:

- *Leave animation to per-frame pushes* — the status quo, which
 [LIMITATIONS §2.2](LIMITATIONS.md#22-animation) correctly called the
 sharpest limitation in the design. It does not merely make animation
 expensive; it makes the *idle* case expensive too, because a plugin cannot
 tell whether its pane is visible without asking.
- *A per-frame callback into the plugin* — general, and it puts a plugin call
 on the frame clock for every animated pane, which is [ADR-V11](#adr-v11)
 inverted. In-process the call is cheap; the *lock the render thread must take
 to make it* is not.
- *Kernel timers that just dispatch an event faster* — a smaller change, and
 it still pays a rebuild + conversion + diff per frame. It optimizes nothing
 that matters.
- *A general tween/easing engine over arbitrary props* — more expressive and
 far more surface. The named kinds cover the enumerated demand; a general
 engine can be added additively if they do not.

---

<a id="adr-v19"></a>

## ADR-V19: Real-time and dense panes are vt100 surfaces

**Choice**: Content that is too fast or too dense for a view tree does not get
a faster view tree. It gets a **terminal grid**: `pty` (the kernel spawns a
process) or `surface` (the kernel allocates a grid and the plugin writes a byte
stream into it). Both render through the same `vt100` + `tui-term` path as a
session pane.

The contract is what makes them usable, and it is specified rather than
implied ([FEATURES-View-Tree.md §3.4](FEATURES-View-Tree.md#34-real-time-surfaces)):

| Concern | Rule |
|---|---|
| Input | A focused grid node is an **input sink** — keys go to the PTY, not to the plugin. Only the kernel-reserved chords (focus cycle, quit) and the node's declared `escape` chord are intercepted |
| Key events | `keyReport: "press"` (default) or `"press-release"`, which pushes the kitty `REPORT_EVENT_TYPES` flag **only while that node is focused** |
| Resize | The kernel resizes the pty and raises `SIGWINCH` from the resolved rect. Plugins do not set `COLUMNS`/`LINES` |
| Lifetime | Bound to the **pane declaration**, not the plugin's VM: it survives suspension and hot reload, and is torn down on pane close or plugin deactivation |
| Rate | Output marks only that pane dirty, at a capped fps — the `detect_output_redraw` path a session pane already uses |

**Why**: The alternative to this is
[ADR-V5](#adr-v5)'s rejected raw-buffer escape hatch, and the objection to
that one was two renderers with two sets of failure modes. A vt100 grid is not
a second renderer. It is *the* renderer — the hottest, most optimized path
thurbox has, already carrying every agent pane, already proven to round-trip
truecolour, half-blocks, and braille.

It also resolves the tension in
[LIMITATIONS §2.1](LIMITATIONS.md#21-graphics-and-dense-cell-art), which noted
the asymmetry and left it standing: the Doom easter egg works because it renders
inside an agent's PTY, and "a plugin cannot do what a process inside a session
can." With `pty`/`surface` it can, without gaining a single new power over the
user's actual terminal.

**Why escapes are safe here and nowhere else.** N1
([CONSTITUTION-DELTA](CONSTITUTION-DELTA.md#n1--the-kernel-renders-plugins-describe))
forbids plugins emitting escape sequences, for good reasons: a leaked escape
corrupts the frame, desynchronizes the cursor, or writes the user's clipboard.
None of those are reachable through a `surface`. The bytes land in a parser the
kernel owns, clipped to a rect the kernel assigned, and what reaches the real
terminal is cells the kernel composited — exactly as with tmux output. N1 is
amended to say what it always meant: **plugins never write to the terminal**;
writing into a kernel-owned emulator is not writing to the terminal.

**Costs, accepted explicitly**:

- `pty` with an arbitrary `command` is arbitrary code execution. It is
 *narrower* than `shell` in one specific way — the output goes into a kernel
 grid the plugin cannot read back, so it is execution-capable but
 exfiltration-limited — and that is the only claim made for it. The install
 prompt treats it as full trust.
- A grid pane costs what a session pane costs. Ten of them cost ten.
- `surface` write throughput is a real channel that needs backpressure; the
 kernel drops frames rather than growing a queue.

**Rejected**:

- *A raw cell buffer in shared memory* — faster and removes the vt100 parse,
 and it reintroduces the memory-safety problem and a second compositing path
 for a saving that does not show up against a parse thurbox already runs on
 every agent pane.
- *Binary frames of styled cells pushed like a view tree* — no new mechanism,
 and it is [ADR-V5](#adr-v5)'s immediate mode with a smaller encoding.
- *Telling plugin authors to run it in a session instead* — which is what v1
 does, and it means a "plugin" that is really a wrapper around
 `thurbox-cli session create`, with no pane, no layout, and no lifecycle.

---

<a id="adr-v20"></a>

## ADR-V20: v2 is delivered on the trunk behind a compile-time gate

**Choice**: Every phase of v2 lands on `main`, continuously, behind a Cargo
feature (`plugins`) that is **absent from stable builds** until it is
deliberately promoted. There is no long-lived `v2` branch. The nightly channel
is a build configuration, not a branch: `main` compiled with the feature on,
published as a prerelease. Full mechanics in
[RELEASE-STRATEGY.md](RELEASE-STRATEGY.md).

The gate moves in three steps, so capability ships before breakage:

| Stage | Cargo `plugins` | Runtime `[features] plugins` | JS runtime | Ships as |
|---|---|---|---|---|
| A — development | off by default | — | user-supplied | nightly only |
| B — experimental | on by default | off by default | user-supplied | a v1 **minor** release |
| C — default | on | on by default | bundled | **2.0.0** |

**Why**:

- **A branch satisfies "stable can't ship v2" and violates "v1 stays
  maintainable".** At ~3.5 commits/day, a 4–6 month refactor is 400–700 commits
  to reconcile, and v2's work *is* the restructuring of `src/app/mod.rs`
  (14,605 lines) and `src/ui/` (36 modules) — exactly the files v1 fixes touch.
  Every merge conflicts inside the file being rewritten. A Cargo feature buys
  the same isolation for a matrix leg.
- **The plumbing a branch needs is plumbing this does not.** No `ci.yml`
  widening (a PR into an unprotected `v2` would have run *no checks at all*), no
  second release workflow drifting from `cd.yml`, no cross-branch checkout in
  `pages.yml`. Each of those was a hazard the branch model had to actively
  mitigate; here they do not exist.
- **Almost all of v2 is additive.** The host, the renderer, the command
  registry, the CLI verbs — none of it removes v1 behavior. Only the pane swaps
  and the extension teardown do. Separating them lets the *value* of v2 reach
  the stable channel at Stage B, months before the *breakage* of v2 at 2.0.0,
  and gives the protocol real third-party exposure while N4's additive-only
  promise is still cheap to keep.
- **Abandonment is cheap and partial success is a real outcome.** Stopping
  during Phases 1–5 is a feature-flag deletion. Stopping after Stage B still
  leaves users a working plugin system on an undamaged v1. A branch makes
  abandonment mean orphaning months of work, which is a bad incentive to have
  pointed at a decision that should stay reversible.

**Costs, accepted explicitly**:

- Two build configurations to keep green — one extra CI matrix leg, and clippy
  twice.
- A migrated pane and its native predecessor coexist from Phase 4 until Phase 6.
  This amends the "no dual implementations" principle; the interim is bounded,
  the pair is self-checking against one insta snapshot, and it buys the ability
  to ship 2.0.0 with a pane still native if one migration stalls.
- One `cfg` branch per migrated pane at its dispatch site in `App::view`,
  removed with the native pane.

**Rejected**:

- *A long-lived `v2` branch merged at 2.0.0* — the instinctive answer, and the
  one the costs above are measured against. Retained as the documented fallback
  with explicit adoption triggers
  ([RELEASE-STRATEGY §10](RELEASE-STRATEGY.md#10-fallback-the-branch-model)), so
  taking it later is a decision rather than a capitulation.
- *A runtime-only flag with no compile-time gate* — simpler, one binary, and it
  puts the whole plugin host and its JS runtime dependency inside the stable
  artifact from day one. That forfeits the artifact-size deferral, widens
  `cargo-deny`'s surface immediately, and makes "stable cannot ship v2" a
  property of a boolean rather than of the compiler.
- *Develop in a separate repository* — maximum isolation, and it makes the
  kernel extraction — which is mostly *moving existing v1 code* — into a
  permanent fork with no path back.

---

<a id="adr-v21"></a>

## ADR-V21: Pane visibility is kernel state; the F1 editor stays kernel

**Choice**: Two carve-outs from [ADR-V1](#adr-v1)'s "every visible surface is a
plugin", both narrow and both mechanically forced.

1. **Pane visibility is kernel-owned.** The manifest seeds it
   (`default_visible`); the kernel owns it thereafter, persists it per pane id,
   and generates `<plugin>.<pane>.{toggle,show,hide}` commands from the
   manifest with no plugin code. `ctx.ui.showPane()` is a request, not the
   source of truth.
2. **The F1 keybinding editor is kernel chrome**, unlike the theme picker,
   settings panel, and repo picker, which are ordinary `overlay`-slot plugin
   panes.

Specified in [FEATURES-Keybindings.md](FEATURES-Keybindings.md).

**Why (1)**: Plugin-owned visibility is circular. A suspended plugin
([ADR-V15](#adr-v15)) cannot show its own pane, and `onPaneVisible:<id>` is
precisely the event meant to wake it. Kernel ownership also preserves a v1
behavior that the migration was otherwise about to drop: `[features]` and the
`F2`/`F3`/`F5`/`F9` panel toggles are **two axes**, not one. Replacing
`[features]` with `thurbox plugin enable|disable` covers "does this surface
exist"; nothing covered "is it on screen right now". Generating the toggle
command means every third-party pane gets the uniform one-key show/hide model
for free instead of each plugin inventing its own.

**Why (2)**: The editor's core operation is capturing the *next physical
keypress*, including chords the kernel would otherwise intercept — `Ctrl+Q`
being the obvious one. A plugin cannot receive a keypress the kernel routes
elsewhere, so a plugin implementation could not rebind the chords most worth
rebinding. This is a structural limit, not a preference, and it is why the
carve-out is exactly one modal rather than "modals are kernel".

**Rejected**:

- *Plugin-owned visibility with a kernel override* — two writers for one
  boolean, and the suspension circularity survives.
- *Each plugin declares its own toggle command* — no kernel change, and the
  uniform panel-toggle model dies: fifteen plugins, fifteen conventions.
- *F1 as a plugin with a kernel "capture mode" escape hatch* — a host call
  that suspends kernel chord interception for one keystroke would be a
  general-purpose input-hijacking primitive, which is a far larger grant than
  the carve-out it avoids.
- *Not persisting visibility (v1's behavior)* — v1 resets `show_*` every
  launch, which is tolerable for four built-in panels and not for an
  open-ended set of third-party panes.

---

<a id="adr-v22"></a>

## ADR-V22: Anchored overlays instead of a floating-element ban

**Choice**: A node may declare `anchor: { to, side, align, flip, offset }`,
positioning itself against another node's resolved rect. Anchored subtrees
resolve in a **second pass** and render into a per-pane **overlay layer**,
z-ordered by pane order then declaration order, clipped to the pane. Nesting is
capped at 3. Specified in [FEATURES-Layout.md §4](FEATURES-Layout.md#4-anchors--the-overlay-layer).

**Why**: The ban was costing more than it saved. Dropdowns, context menus,
tooltips and inline compose boxes are ordinary TUI furniture, and v1 already
ships one — `render_compose_inline` floats a comment box at a diff line and
flips above or below as room allows. v2 had answered that with a bespoke
`inlineAt` slot on the `diff` node: a point fix that left every other pane with
no route, and that this ADR deletes.

The three properties that made the ban attractive survive:

- **Focus uniqueness.** An anchored subtree belongs to its pane and is not a
  focus target, so exactly one pane still holds focus.
- **Single-pass layout for trees that do not use it.** Two passes are paid by
  anchors, not by everyone.
- **Determinism.** Z-order is positional, not a `z-index` free-for-all.

What genuinely changes is one invariant: "nothing overlaps" becomes "the base
layer never overlaps; the overlay layer may, and is strictly ordered." That is
narrow enough to assert in the monkey test.

**Rejected**:

- *Keep the ban* — the status quo, which pushes every plugin toward an
  `overlay`-slot modal (wrong shape for a dropdown) or inline expanding rows
  (often better, sometimes not) and keeps `diff.inlineAt` as a permanent
  special case.
- *A `z-index` property* — familiar and unbounded; positional ordering is
  enough for menus and cannot be abused into layering wars.
- *Escaping the pane rect in 2.0* — needed for a dropdown at a narrow pane's
  edge, and it requires cross-pane z-ordering plus a story for what happens
  when the owning pane is hidden mid-interaction. Deferred deliberately.

---

<a id="adr-v23"></a>

## ADR-V23: Pane geometry is a workspace tree; slots are a preset

**Choice**: Space is divided by a **tree of splits** — branches are horizontal
or vertical splits, leaves are panes or tab groups, each carrying
`size`/`weight`/`min_*`. It persists as an optional
`~/.config/thurbox/layout.toml`. The manifest's `slot` becomes an
**auto-placement hint** for panes the tree does not name, and the five slots
ship as the **default preset** — a synthesized tree reproducing v1's
`PanelAreas` exactly. Specified in
[FEATURES-Layout.md §2](FEATURES-Layout.md#2-the-workspace-tree).

**Why**: The slot model answered "where does a pane go" with five fixed
answers, and [LIMITATIONS §1.2](LIMITATIONS.md#12-pane-geometry) listed six
things that needed a sixth, seventh and eighth. A tree answers all of them at
once — full-width
spanning regions, 2×2 grids, nested splits, runtime reordering, header docking
— because they are all just shapes of the same structure. Adding slots one at a
time would have been more total work for a worse result.

It is also arguably a **simplification**: `compute_layout`'s 10 fixed `Rect`
fields and 9-argument signature collapse into one recursive structure, and
"tabbed when several are visible" stops being a property of the `center` slot
and becomes a leaf kind available anywhere.

Zero-config behavior is unchanged, which is the constraint that made this
adoptable: a user who never writes `layout.toml` sees exactly the v1 layout,
and a plugin author still declares `slot = "right"` and thinks no further.

**Deferred**: interactive split resize (drag a border, tmux-style). The tree
makes it possible — a drag writes back a `size` — but it needs border hit
regions, a persistence policy for transient drags, and keyboard equivalents.
The file is editable in the meantime.

**Rejected**:

- *Add slots as needed* — cheapest per request, and it ends with a dozen
  ad-hoc slot names encoding positions that a tree expresses structurally.
- *Expose the tree only internally, keep slots as the public API* — preserves
  the simpler mental model, and then "I want a 2×2 dashboard" has no answer at
  all, which is the complaint that started this.
- *A CSS-grid-style declarative area map* — more expressive for fixed
  layouts, worse for the nested-split and tab-group cases that terminal
  workspaces actually use.
- *Let tmux own pane geometry* — the obvious question for this project, since
  thurbox already runs on tmux and tmux already has splits, resize, zoom and
  named layouts its users know. Rejected because it dismantles the product:
  each pane would need its own process rendering into its own PTY, so a plugin
  pane stops being a view tree and becomes a full TUI application — no shared
  theming, no unified focus, no view-tree contract, and no demand-driven
  redraw loop. It also has no psmux/Windows story. thurbox is one coherent TUI
  *over* tmux, not a tmux configuration.
- *Adopt Zellij's KDL layout language* — familiar to exactly the audience
  thurbox targets, and battle-tested. Rejected on format consistency: every
  other config file in the repo is TOML, and a second format for one file
  costs more than the familiarity buys.
- *A `[[layoutProviders]]` contribution point, so a plugin can replace the
  solver* — considered and rejected, despite the pull of
  [ADR-V1](#adr-v1)'s "everything but six things is a plugin". Layout is
  already one of those six, and for reasons that hold on inspection:
  - The solver is **how the global invariants are enforced** — focus
    uniqueness, panes never overlapping, kernel chrome placement. Delegating
    enforcement of an invariant to the code it constrains is backwards, and it
    would mean validating every returned rect precisely because the boundary
    cannot be trusted.
  - **The built-in solver has to be complete regardless.** It computes the
    first frame before any plugin exists, and it is the only sane fallback on
    a provider crash or deadline. A provider therefore adds a *second* path
    that must be as good as the first, rather than replacing work.
  - It would **freeze layout into the public protocol**. Under
    [N4](CONSTITUTION-DELTA.md#n4--the-plugin-api-is-additive-within-a-major-version)
    the request/response shape becomes additive-only for the life of the major
    version, so the kernel could never restructure how it lays out.
  - The flexibility people actually want is **configuration, not execution**.
    `layout.toml` already makes geometry user-editable data; a process
    boundary buys expressiveness nobody has asked for at the cost of a
    total-loss failure mode.
