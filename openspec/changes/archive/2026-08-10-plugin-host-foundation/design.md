## Context

See `proposal.md` — Why. This design covers only the kernel-side machinery: a
script runtime, a manifest format, discovery, a lifecycle, and capability
enforcement. Nothing here draws a pane.

Four properties of the existing codebase constrain the approach:

- **The architecture test is an allowlist.** `tests/architecture_rules.rs`
  fails on any module under `src/` without a `ModuleRules` entry, and checks
  references in every form including fully-qualified paths. A new module is a
  declared architectural decision, not a free addition.
- **`session/` is the pure-data sink.** It may reference no crate-internal
  module. v1 already puts declarative config types there — `AgentDef`,
  `HostDef`, `ExtensionDef` — parsed by loaders that live one layer up.
- **The render loop is demand-driven.** `run_loop` paints only when
  `App::needs_redraw` or the 250 ms floor fires. Anything that blocks the UI
  thread is a regression in a property that was expensive to win.
- **The real MSRV is already 1.86, not the documented 1.75.** `ratatui 0.30`
  declares `rust-version = 1.86.0`; `clippy.toml` still says `msrv = "1.75"`
  and `Cargo.toml` declares no `rust-version` at all, so nothing enforces the
  claim. This matters because the runtime dependency raises the floor again.

## Goals / Non-Goals

**Goals:**

- Settle the module boundary for the plugin host before any pane depends on it,
  including its entry in the architecture allowlist.
- Make every spec requirement testable without a terminal, a tmux server, or a
  network — the plugin host must be unit-testable the way `session/` is.
- Keep stable builds byte-identical: no dependency, no code, no size delta when
  the `plugins` feature is off.
- Pick a threading model that makes "plugins never block the render loop" a
  structural property rather than a discipline.

**Non-Goals:**

- Designing the host API surface. Only the lifecycle's own bindings exist here;
  every real binding arrives with the change that needs it.
- Designing the view tree, pane slots, or anything a plugin renders.
- Optimizing plugin call throughput. Correctness and isolation first; there is
  no pane in the render path yet to make it matter.

## Decisions

### D1: Luau via `mlua`, with the VM created on its owning thread

**Decision.** Depend on `mlua 0.12` with features `luau` and `vendored`, as an
optional dependency activated by `plugins`. Do **not** enable mlua's `send`
feature. Each plugin thread constructs its own `Lua` after it starts, so the VM
value never crosses a thread boundary.

**Why.** mlua is the only maintained Rust binding with first-class Luau
support, and it exposes exactly the three primitives the runtime spec requires:
`Lua::sandbox` (read-only globals, per-VM isolation), `set_interrupt` (the
instruction budget), and `set_memory_limit` (the memory ceiling). Building
those on a raw FFI binding would be reimplementing the reason to choose Luau.

Not enabling `send` is the load-bearing half. mlua's `send` feature makes `Lua`
`Send` by wrapping its state in a lock, which costs on every call and — worse —
makes it *possible* to hold a VM handle on the UI thread. Leaving `Lua` as
`!Send` means the compiler enforces that a VM is only ever touched by its own
thread. The "plugins never block the render loop" requirement stops being a
review rule and becomes a type error.

**Alternatives considered.**

- *Lua 5.4 via mlua.* Familiar to more contributors and the same embed cost.
  Rejected because Luau's sandbox mode, interrupt callback, and memory limit
  are built for exactly this use; on 5.4 the sandbox would be hand-rolled from
  environment scrubbing, which is the shape that historically leaks.
- *An in-process JS runtime.* Better library ecosystem and far more people can
  write it. Rejected here because the isolation story is weaker per-VM and the
  binary-size cost is an order of magnitude larger against a single-binary
  install constraint.
- *WASM guests.* Genuine fault isolation. Rejected for the same reason the
  proposal's non-goals list native code: the toolchain burden lands on every
  plugin author, for an isolation win that per-VM bounds already approximate.
- *Enabling `send` and pooling VMs on a shared runtime.* Fewer threads. Rejected
  because it re-admits UI-thread VM access and buys nothing until plugin counts
  are far higher than they will be.

### D2: One OS thread per plugin, driven by a request channel

**Decision.** Each loaded plugin owns a dedicated OS thread — not a tokio task.
The thread constructs the VM, then services a `mpsc` request channel; each
request carries a one-shot reply sender. The host side of a call is always
non-blocking from the UI thread's perspective: requests are enqueued, replies
are drained on the existing tick.

**Why.** A tokio worker is a shared resource, and plugin code is arbitrary
blocking CPU work with no yield points. A plugin that loops forever would park
a runtime worker and degrade unrelated async work. A dedicated OS thread makes
the blast radius of an infinite loop exactly one thread, which is what the
runtime spec's "host continues to draw frames" scenario demands. This mirrors
what v1 already does for PTY reads (`spawn_blocking`) — blocking work gets its
own thread rather than the async pool.

**Trade-off accepted.** One thread per plugin means thread count scales with
plugin count. With the bundled set that is single digits, and threads are cheap
relative to a VM.

**Alternative considered.** A shared pool of N worker threads multiplexing all
plugins. Rejected: a single hung plugin would starve every plugin sharing its
worker, and the interrupt-based budget only bounds a call after it trips —
until then, other plugins on that worker are stalled.

### D3: The manifest is pure data in `session/`; everything else is a new `plugin` module

**Decision.** Split across two places, following existing precedent:

- `src/session/plugin_manifest.rs` — the manifest types, parsing, and
  validation. Pure data, no crate-internal references, matching how
  `AgentDef` / `HostDef` / `ExtensionDef` already live there.
- `src/plugin/` — a new top-level module for the runtime, discovery, lifecycle,
  and capability enforcement, with the allowlist entry
  `allowed: &["session", "paths"]`. No `ui`, no `app`, no `git`, no `agent`.

**Why.** The manifest spec requires parsing with no runtime and no side
effects, which is precisely what `session/`'s "no crate-internal references"
rule already guarantees structurally. Putting it there means the architecture
test enforces the spec requirement for free. The runtime half is side-effecting
(threads, VMs, filesystem) and sits at `agent`'s altitude — below `app`, above
`session`.

`plugin` deliberately does **not** get `agent` or `storage`, even
path-qualified. Nothing in this change needs them, and adding them later is a
reviewed decision rather than an inherited one.

**Alternative considered.** Putting everything in `src/plugin/`, manifest
included. Rejected because it would let manifest parsing quietly grow
side effects, and the spec's "readable without a runtime" requirement would
then rest on discipline instead of the allowlist.

### D4: Capability enforcement by environment construction

**Decision.** Grant capabilities by building each VM's plugin-visible module
table from the granted set, then sandboxing the VM so globals are read-only.
An undeclared capability's binding is never inserted. There is no runtime check
inside a binding that consults a permission list.

**Why.** The capabilities spec requires absence rather than refusal, and the
reason is that refusal is a check someone can forget to write. If the only
place a binding can come from is the table built at VM construction, then "did
we check?" has one answer for every binding, decided once. Luau's sandbox mode
makes the resulting environment read-only, which closes the obvious escape of
reassigning entries.

**Alternative considered.** A single host dispatch function that validates the
requested capability per call. Rejected: it puts the enforcement point inside
the thing being enforced, and it leaks the vocabulary of every capability to
every plugin, turning the manifest into a hint.

### D5: Discovery sources are an ordered list with last-wins override

**Decision.** Two sources, in order: a compile-time bundled registry (empty in
this change — the type and ordering exist, the contents arrive with the first
bundled plugin), then `~/.config/thurbox/plugins/` resolved through
`paths::resolve`. Later source wins on a name collision; a collision *within*
one source rejects both.

**Why.** The user plugin directory belongs in the config dir because that is
where v1 already puts user-installed extension manifests
(`~/.config/thurbox/extensions/`); the data dir holds materialized built-in
assets. Routing through `paths::resolve` means `THURBOX_CONFIG_DIR` already
redirects it, so the sandbox scripts and the acceptance harness get isolation
with no new mechanism.

Last-wins is what makes "copy the bundled plugin, edit it, and it takes over"
work, which is the cheapest possible path from user to plugin author. Within
one source there is no principled tiebreak, so rejecting both and reporting it
is the honest outcome — silently picking one by directory order would be the
non-deterministic behavior the discovery spec forbids.

### D6: No `rust-version` bump hidden inside the feature

**Decision.** `mlua 0.12` declares `rust-version = 1.88`, above `ratatui`'s
1.86. Set `rust-version = "1.86"` in `Cargo.toml` for the crate as it stands,
correct `clippy.toml`'s stale `msrv = "1.75"` to match, and document that the
`plugins` feature additionally requires 1.88. CI's plugin-enabled job pins a
toolchain that satisfies it.

**Why.** The documented 1.75 is already false — the crate has not built on 1.75
since the ratatui 0.30 upgrade. Adding a dependency that raises the floor
further while the declared floor is fiction would compound the problem. Cargo
has no way to express "this feature needs a newer compiler", so the honest
encoding is an accurate base `rust-version` plus a documented feature-level
requirement enforced by the CI job that builds it.

**Trade-off.** A contributor on 1.86 can build stable thurbox but not the
plugin feature, and will learn that from a compiler error rather than a clean
message. Accepted: the alternative is holding the runtime hostage to a floor
nothing else respects.

### D7: Bounds are configured per plugin, with defaults that are not silently generous

**Decision.** The instruction budget and memory ceiling are per-plugin values
with host-chosen defaults, sourced from the host rather than the manifest. A
plugin cannot raise its own bounds.

**Why.** Self-declared limits are not limits. Sourcing them from the host keeps
the manifest a description of what a plugin *does*, not what it is *allowed*,
which is the same reason capabilities are validated against a closed
vocabulary.

## Risks / Trade-offs

- **The instruction budget cannot distinguish slow from stuck.** A legitimately
  expensive plugin call and an infinite loop both trip it. → Bound generously
  and treat the budget as a liveness guard, not a fairness mechanism. The real
  fix — moving long work off the call path — belongs with the change that
  introduces long work.

- **Interrupt-based termination leaves the VM in an undefined state.** A call
  killed mid-execution may have mutated plugin state halfway. → Terminate the
  whole VM on a budget or memory failure rather than resuming it, and mark the
  plugin `failed`. A plugin that trips a bound is a bug to fix, not a state to
  recover.

- **`vendored` adds a C compiler to the plugin build.** → Confined to the
  `plugins` feature; stable builds and the default CI matrix are untouched. The
  plugin-enabled CI job is where a missing toolchain surfaces.

- **`cargo deny` will see new licenses from the vendored Luau sources.** → Part
  of the dependency task, not a discovery at release time.

- **Per-VM isolation is not fault isolation.** A soundness bug in mlua or Luau
  itself takes the process down, and the specs' containment guarantees rest on
  that boundary holding. → Accepted and stated in the proposal's cost list;
  this is the reason plugins may carry no native code, since that would remove
  the one boundary being relied on.

- **The threading model is easy to erode.** The next contributor who wants a
  synchronous plugin call from the UI thread will find `!Send` inconvenient. →
  The `!Send` VM makes the erosion a compile error rather than a review catch,
  which is why D1 declines the `send` feature.

## Migration Plan

Nothing to migrate — this change adds a feature that is off by default and
touches no existing behavior.

**Deployment.** All new code is behind `#[cfg(feature = "plugins")]`. The
default build, the default test run, and every release artifact are unchanged.
A second CI job builds and tests with `--features plugins`, so both
configurations are verified on every commit.

**Rollback.** Removing the feature from a build removes the code path entirely.
Reverting the change removes one optional dependency and one new module; no
persisted state, no config file, and no user-visible surface has been created
that would outlive it.

**Verification that stable is unaffected.** The default `cargo tree` must not
contain `mlua`, and the default test suite must pass unchanged.

## Open Questions

- **How is a plugin's failure surfaced to the user?** Discovery and lifecycle
  failures are recorded and inspectable per the specs, but *where* a user reads
  them — a log line, a status toast, a `plugin status` verb — depends on the
  CLI surface, which is a later change. Deferrable: the recorded shape is
  specified here, only its presentation is open.

- **What are the default numeric values for the instruction budget and memory
  ceiling?** They need a real plugin doing real work to calibrate against, and
  no such plugin exists yet. Deferrable: D7 fixes where the values come from
  and who may change them, which is the part that would be expensive to revisit.
