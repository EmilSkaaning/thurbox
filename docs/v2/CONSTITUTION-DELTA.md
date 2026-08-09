# Thurbox v2 — Constitution and ADR Delta

[`docs/CONSTITUTION.md`](../CONSTITUTION.md) holds rules that are
**non-negotiable and automatically enforced**. v2 changes what some of them
mean. This document states each change precisely, so the constitution is
amended deliberately rather than eroded by implementation.

Rule of thumb applied throughout: **a rule survives if it can still be
enforced by a script.** A rule that v2 makes unenforceable is retired, not
weakened into advice.

---

## 1. Amended principles

Ordered by rule number.

### Rule 1 — Crash-free operation

**v1**: errors are displayed in the UI, never via panics; the only panic path
is the terminal-restore hook.

**v2**: unchanged for the kernel, and extended to cover the new failure mode.
**A plugin fault must never reach the user as a broken frame.** A plugin that
errors, loops, or exhausts its memory limit renders its pane's last tree with a
visible error state and restarts with backoff — errors surface as `Result`s,
runaway loops are cut by Luau's interrupt handler, and allocation is capped per
VM ([ADR-V4](ARCHITECTURE.md#adr-v4)).

**This rule is now conditional, and the condition is
[C2](ARCHITECTURE.md#adr-v2).** Plugins run in the thurbox process, so a
segfault in native code would take the TUI down and no supervisor could catch
it. The rule holds *because* plugins may not carry native code — not because
the architecture makes it impossible. That is a genuine weakening relative to
the process model, and it is the one place v2 trades a Constitution guarantee
for something else.

**What the condition does not cover, and what happens then.** A memory-safety
bug in the Luau VM itself is in-process and uncatchable — C2 bounds the
*surface* to one vendored dependency, it does not eliminate it. The residual
blast radius is smaller than it looks, and for the v1 reason: agent sessions
live in tmux, not in thurbox. A kernel crash loses the TUI and the frame; it
does not lose a session, a worktree, or an agent's turn in progress, and a
relaunch re-adopts every pane. That is the same recovery v1 already relies on
for a panic, which is why v2 adds no watchdog or supervisor process — one would
restore nothing that a relaunch does not.

**Enforcement**: conformance tests with a fixture plugin that errors on init,
exceeds deadlines, exhausts its memory limit, and returns a malformed tree.

---

### Rule 2 — Module isolation

**v1**: `session → agent → ui → app`, with `agent` and `ui` never importing
each other.

**v2**: unchanged in spirit; one module family added.

```text
session   ← pure data types (now including view-tree types)
agent     ← session
plugin    ← session (+ paths/shell); NEVER ui, git, app
ui        ← session + app model/view state; NEVER agent, git, plugin
app       ← coordinator
```

`plugin` is a side-effect layer beside `agent`. The view-tree types live in
`session` so `ui` can render them without importing `plugin` — the same reason
`session::review` exists so `ui` can render diffs without importing `git`.

**Enforcement**: unchanged — `tests/architecture_rules.rs`, extended with the
`plugin` allowlist entry.

---

### Rule 3 — Zero-warning policy

**v1**: clippy and rustdoc with warnings as errors; rumdl for markdown.

**v2**: extended to Luau. `luau-analyze` in strict mode and the plugin test
suite gate CI for `plugins/` and the bundled `@thurbox` modules with the same
zero-tolerance rule.

**Enforcement**: new CI jobs and pre-commit stages, landing in
[MIGRATION Phase 0](MIGRATION.md#phase-0--foundations-no-behavior-change) —
before the first Luau PR, so that PR does not also have to carry the
toolchain. The rule is unchanged; its scope grew.

---

### Rule 4 — Permissive licenses only, and Rule 5 — Zero known vulnerabilities

**v1**: `cargo-deny` over the Rust dependency tree.

**v2**: two supply chains, two gates, same standard.

| Chain | Gate |
|---|---|
| Rust | `cargo deny check advisories bans licenses sources` (unchanged) |
| Luau VM (`mlua`) | An ordinary Cargo dependency — already inside `cargo-deny`'s advisory, license and ban checks |
| Plugin code | No package manager, so no transitive supply chain to audit ([ADR-V2](ARCHITECTURE.md#adr-v2)) |

**This got simpler, not harder.** An earlier draft accepted a second supply
chain — npm for plugin packages, plus a vendored JavaScript runtime whose CVE
response thurbox would own. Embedding the VM
([ADR-V3](ARCHITECTURE.md#adr-v3)) means there is one dependency tree, already
governed by the existing gate.

**Enforcement**: unchanged — the existing `cargo-deny` job.

---

### Rule 7 — TEA as the single architectural pattern

**v1**: "No ad-hoc event handlers, no component-local state, no callback
chains."

**v2**: TEA becomes **hierarchical**. The kernel keeps one model. Each plugin
keeps its own model in its own VM, updated by its own reducer, rendered by its
own pure `render`. The prohibition holds at both levels; there are two levels.

The clause that changes is "no component-local state". Its purpose was to
prevent panes mutating shared `App` state through hidden paths. Across a VM
boundary that hazard cannot occur: plugin state lives inside a `mlua::Lua`
state the kernel never reads, reachable only through its own reducer, and the
only thing crossing is events in and view trees out — which is TEA, not an
escape from it.

**Enforcement**: the host bindings themselves. Nothing bound into a plugin VM
mutates kernel state directly; every mutation is a typed host call, and a
capability the plugin lacks has no binding to call ([ADR-V4](ARCHITECTURE.md#adr-v4)).
`render` purity is enforced structurally — it runs with the host tables
unavailable, so a host call in `render` is an error rather than a convention.

---

### Rule 8 — Backend-first session model

**v1**: sessions run through `SessionBackend`, backed by tmux; never mocked,
emulated, or screen-scraped.

**v2**: unchanged, and reinforced. The terminal grid is a kernel surface
([ADR-V6](ARCHITECTURE.md#adr-v6)) precisely so no plugin can interpose
itself between tmux and the screen. **Plugins may never emulate a terminal**
— that is added as an explicit clause. A `surface`
([ADR-V19](ARCHITECTURE.md#adr-v19)) is consistent with it rather than an
exception to it: the plugin *produces bytes*, the kernel emulates. A plugin
parsing vt100 itself, or rendering a session's grid from its own parse, is the
prohibited thing and stays prohibited.

---

### Rule 9 — Logging never touches stdout

**v1**: stdout belongs to the TUI; logs go to `thurbox.log`.

**v2**: extended to plugins. Plugins share the kernel's process, so its stdout
is the TUI's for exactly the v1 reason — a plugin write would paint over the
frame. `print` is bound to a kernel function that routes to `thurbox.log`,
tagged with the plugin name, rather than to the real stdout; `ctx.log()` is the
sanctioned path and carries a level.

**Enforcement**: the VM is created with no `io` library and a replaced `print`,
so there is no binding through which a plugin can reach the real stdout — the
same absence-not-checking pattern as capabilities.

---

### Rule 10 — Test-driven development

**v1**: Red, Green, Refactor; tests written before or alongside
implementation.

**v2**: unchanged, applied to both languages. Additionally, a **migrated pane
must pass the tests its Rust predecessor passed** before the Rust
implementation is deleted — the insta snapshots and monkey invariants are the
migration's acceptance criteria, not a separate exercise.

Under [ADR-V20](ARCHITECTURE.md#adr-v20) this is stronger than it sounds. The
two implementations coexist from Phase 4 until Phase 6, so the same snapshot is
asserted against **both** in the same test run, and the plugin's fidelity is a
live check rather than a comparison against a deleted predecessor.

**Enforcement**: the existing insta and monkey suites, parameterized over both
implementations while both exist.

---

## 2. Unchanged principles

Rules **6** (conventional commits), **11** (deterministic CI — scripts over
LLMs), and **12** (tag-based versioning) are untouched by v2.

Two notes on rule 12, both mechanism rather than exception:

- `cog bump --auto` cannot cross a major boundary on its own, so 2.0.0 ships via
  the explicit-version release dispatch — a documented v1 mechanism.
- Nightly prereleases are tagged `nightly-YYYY-MM-DD`, deliberately **not**
  semver, so they never participate in version computation. Under
  [ADR-V20](ARCHITECTURE.md#adr-v20) nightlies are built from `main`, which
  makes their tags immediately reachable, so this is load-bearing rather than
  cosmetic
  ([RELEASE-STRATEGY §4.2](RELEASE-STRATEGY.md#42-tag-naming-and-why-it-is-not-semver)).

---

## 3. New principles proposed for v2

Each is stated with its enforcement mechanism, per the constitution's own
admission rule ("if it can't be enforced, it doesn't belong here").

### N1 — The kernel renders; plugins describe

No plugin writes to the user's terminal, and none receives a drawable buffer.
The kernel is the sole renderer.

The one place a plugin may emit escape sequences is **into** a kernel-owned
vt100 emulator — a `surface` node ([ADR-V19](ARCHITECTURE.md#adr-v19)). That is
not an exception: the bytes are parsed by the kernel, clipped to a rect the
kernel assigned, and what reaches the terminal is composited cells, exactly as
with tmux output. Escapes in the *view tree* remain forbidden outright.

**Enforcement**: `text` node content is escape-stripped when the tree is
converted at push time, with a test; `ctx.surface.write` is gated on the `pty`
capability — the binding is absent without it — and its bytes
never leave the emulator, with a test asserting an OSC 52 write through a
`surface` sets no clipboard.

### N2 — A frame never waits on a plugin

Rendering always paints from cached view trees. No call into a plugin VM sits
on the critical path of a frame — the render thread never acquires a plugin's
`Lua` state.

**Enforcement**: `perf_*` counter tests assert frames continue while a fixture
plugin is unresponsive, plus a monkey invariant.

### N3 — Capabilities are declared, gated, and shown

Every privileged **host call** is gated by a manifest declaration, enforced at
one place — the bind step that builds a VM's host tables — and displayed at
install time.

**Scope.** The rule binds everything a plugin can reach, which under
[ADR-V2](ARCHITECTURE.md#adr-v2) is everything full stop: Luau ships no
filesystem, network or process access, so `fs`, `net` and `shell` are host
bindings like the rest rather than ambient powers a manifest can only describe.
All sixteen capabilities are enforced. An earlier draft, written against an
out-of-process JavaScript runtime, had to except those three; that exception is
withdrawn ([SECURITY.md §3](SECURITY.md#3-resolved--fs-net-and-shell-are-now-enforced)).

**Enforcement**: capability tests per host binding; a coverage test asserting
every privileged binding is reachable only under its grant; and a test
asserting an ungranted plugin's VM has no such global
([SECURITY.md §4](SECURITY.md#4-plugin-environment)).

### N4 — The plugin API is additive within a major version

New node types, new optional command arguments, and new events are allowed.
Removing or retyping is a protocol major bump. Unknown nodes render as a
placeholder; unknown fields are ignored.

**Enforcement**: a golden protocol schema checked into the repo, with a CI
diff that fails on a non-additive change without a major version bump.

---

## 4. v1 ADRs affected

| ADR | Effect in v2 |
|---|---|
| ADR-1 (TEA) | Amended — hierarchical, see rule 7 above |
| ADR-2 (SessionBackend + vt100 + tui-term) | Unchanged; becomes kernel |
| ADR-3 (tokio) | Unchanged; plugin supervision joins the runtime |
| ADR-4 (input translation) | Unchanged; kernel keeps owning key → bytes |
| ADR-5 (responsive breakpoints) | Superseded by the layout solver and manifest `min_width` |
| ADR-6 (file-based logging) | Extended to plugin stderr |
| ADR-8 (SQLite) / ADR-7b (multi-instance sync) | Unchanged; `plugin_kv` added ([ADR-V9](ARCHITECTURE.md#adr-v9)) |
| ADR-9 (flat session list) | Moves into the session-list plugin's design |
| ADR-11 / ADR-12 / ADR-13 (backends, tmux, SSH/WSL) | Unchanged; kernel |
| ADR-14 (centralized theme) | Extended — theme tokens become the plugin styling vocabulary |
| ADR-15 (headless CLI as separate binary) | Unchanged; gains the command registry |
| ADR-19 (declarative agent definitions) | Unchanged; plugins may contribute `[[agents]]` |
| ADR-20 (agent-agnostic extensions) | **Retired** — superseded by [ADR-V8](ARCHITECTURE.md#adr-v8) |
| ADR-21 (declarative extension manifests) | **Retired** — superseded by [ADR-V8](ARCHITECTURE.md#adr-v8) |
| ADR-22 (`App` decomposition) | Superseded — the kernel/plugin split is the decomposition |
| ADR-P1–P12 (performance) | Preserved; [ADR-V11](ARCHITECTURE.md#adr-v11) exists to keep them true |
