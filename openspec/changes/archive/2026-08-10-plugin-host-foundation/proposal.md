## Why

Every user-visible surface in thurbox v1 is compiled in. Adding one pane means
editing roughly ten parallel tables in lockstep — `App`'s 80 fields,
`InputFocus`, `Action`, `KeyContext`, `FeatureFlags`, `PanelAreas`,
`ClickAction`, `SettingsField`, `focus_ring`, and the pinned acceptance
snapshots — spread across a 14,605-line `src/app/mod.rs` and 36 `ui/` modules.
The consequence is that nobody outside the project can add a pane, no pane can
be tried out and thrown away in an afternoon, and v1's ~4,200 lines of
`extensions/` machinery buy the ability to script agents but zero ability to
draw anything.

v2 answers that by making thurbox a plugin host. This change is the kernel-side
foundation that must exist before any pane can be a plugin: the runtime that
loads plugin code, the manifest that declares what a plugin provides, and the
capability model that bounds what it may do. It ships no UI and changes no
user-visible behavior — deliberately, so the contracts everything else depends
on can be settled and tested in isolation.

## What Changes

- **New Cargo feature `plugins`**, off by default and absent from stable
  release builds. All code introduced here is behind `#[cfg(feature =
  "plugins")]`, so v1 keeps releasing on its existing cadence from the same
  `main` with no behavior delta and no binary-size delta.
- **A Luau runtime embedded in-process** via `mlua` with the `luau` backend.
  Each plugin gets its own VM on its own thread; VMs never share Lua state.
  Faults, panics, and runaway execution are contained to the offending plugin
  and reported as a plugin-level error rather than taking the TUI down.
- **A plugin manifest format** (`plugin.toml`), parsed as pure data with no VM
  involved. A manifest declares the plugin's identity, its API version, the
  panes/commands/keybindings it provides, and the capabilities it requests. The
  kernel can therefore enumerate everything a plugin offers before executing a
  single line of its code.
- **Plugin discovery** over a defined, ordered set of sources — plugins bundled
  into the binary, then the user plugin directory — with deterministic
  precedence and duplicate detection.
- **A plugin lifecycle**: discovered → loaded → initialized → (running) →
  stopped, with a defined transition for load failure at each step. A plugin
  that fails to parse, fails to compile, or errors in `init` is recorded as
  failed and skipped; it never blocks startup or another plugin.
- **A declared capability model**, enforced at the host binding boundary. A
  plugin that did not request a capability cannot reach the host function that
  needs it — the binding is absent from its VM's environment rather than
  present-and-refusing.
- **Execution bounds per plugin**: an instruction-count interrupt and a memory
  ceiling, so a plugin that loops forever or allocates without limit is
  terminated instead of hanging or OOMing the process.
- **No UI surface, no CLI surface, no persistence schema change.** Panes,
  rendering, the view tree, host API breadth, hot reload, and the headless
  service half are each their own later change.

## Capabilities

### New Capabilities

- `plugin-host/runtime`: embedding the Luau VM — VM and thread per plugin,
  isolation between plugins, execution bounds (instruction budget, memory
  ceiling), fault containment, and the error surface a failing plugin produces.
- `plugin-host/manifest`: the `plugin.toml` schema, its parse and validation
  rules, the API-version compatibility check, and the guarantee that a manifest
  is readable as pure data without starting a VM.
- `plugin-host/discovery`: where plugins are found, in what order, how
  precedence and duplicate ids resolve, and what a malformed or unreadable
  plugin directory does to startup.
- `plugin-host/lifecycle`: the plugin state machine, what `init` receives and
  may return, ordering guarantees across plugins, shutdown, and the terminal
  states for each failure mode.
- `plugin-host/capabilities`: the capability vocabulary, how a manifest
  requests capabilities, how the host enforces them at the binding boundary,
  and what an undeclared access attempt does.

### Modified Capabilities

None. This is the first change in the repository; `openspec/specs/` is empty
and no existing v1 behavior changes.

## Non-goals

- **No panes and no rendering.** Nothing a plugin returns reaches the screen in
  this change. The view tree and the pane slot model are a separate change.
- **No breadth in the host API.** Only what the lifecycle itself requires. The
  session, storage, git, and process host bindings come with the changes that
  need them, each gated by its own capability.
- **No hot reload.** Plugins load at startup and stop at shutdown. Reload on
  save is a later change, and the lifecycle state machine here is written to
  admit it without redesign.
- **No headless service half.** Plugins do not run under `thurbox-cli` or the
  automation heartbeat yet.
- **No plugin installation, registry, or `thurbox plugin` CLI verbs.**
  Discovery reads directories that already exist; nothing puts them there.
- **No migration of any v1 pane.** The tasks pane, file viewer, code review,
  automations pane, and session list stay exactly as they are, compiled in.
- **No third-party plugin support.** The capability model is built here, but
  the threat model, signing, and install path that would make running someone
  else's plugin defensible are explicitly out of scope.

## Impact

**Dependencies.** Adds `mlua` (with the `luau`, `vendored`, and `send`
features) as an optional dependency activated by the `plugins` feature. Luau
vendors as C source, so the build gains a C compiler requirement on the
`plugins` feature only — stable builds are unaffected. `cargo deny` needs the
new licenses reviewed and `Cargo.lock` regenerated.

**New code.** A `src/plugin/` module tree, entirely new. Its place in the
architecture allowlist must be declared in `tests/architecture_rules.rs` or the
architecture test fails by design. The manifest types are pure data and belong
under `session/` per the existing dependency rules (`session` may reference no
crate-internal module); the runtime, discovery, and lifecycle are side-effect
code and sit at the `agent` layer's altitude — no `ui`, no `app`, no `git`.

**Build and CI.** A `plugins`-enabled build and test job, plus a Luau toolchain
for linting and type-checking plugin fixtures. The default job set continues to
build and test without the feature, so a regression in either configuration is
visible.

**Runtime.** None in stable builds. With `plugins` enabled and no plugins
present, startup cost must stay within the existing first-frame budget; the
threads and VMs are created per discovered plugin, so the no-plugin case
allocates nothing.

**Docs.** `docs/` describes the shipping product and stays authoritative for
v1. The v2 contracts live in `openspec/specs/` as they land; this change adds
no `docs/v2/` prose.
