## Context

See `proposal.md` — Why. The foundation change built the host but wired it to
nothing; this one puts it on the startup path of both binaries and adds the
read-only CLI that reports it.

Three existing facts shape the approach:

- **`src/app/mod.rs` is ~14.6k lines, and shrinking it is the point of v2.**
  Anything that can avoid becoming an `App` field should.
- **The render loop is demand-driven.** `run_loop` paints when dirty or on the
  250 ms floor. Startup work that blocks the UI thread delays the first frame
  directly.
- **`cli` may reach `agent` only through fully-qualified paths, never `use`.**
  The allowlist encodes that headless→backend dependencies stay visible at each
  call site.

## Goals / Non-Goals

**Goals:**

- Start and stop the host in both binaries without adding a field to `App`.
- Make a slow or hanging plugin structurally unable to delay the first frame.
- Give a user a truthful answer to "why isn't my plugin running?", including
  failures that only appear when a plugin is actually run.

**Non-Goals:**

- Rendering anything a plugin produces.
- Any mutation verb. Everything here reads.
- Making the CLI fast enough to call in a loop; these are diagnostic verbs.

## Decisions

### D1: `main.rs` owns the host, not `App`

**Decision.** The TUI's `PluginHost` lives in `main.rs` as a local, alongside
the terminal and the backend registry. `App` gains no field, no method, and no
import.

**Why.** Nothing in the model or the view consumes plugins yet — the host has
no rendering surface until the view-tree change. Threading it into `App` now
would add a field to the exact struct v2 exists to shrink, and would put the
plugin module inside the coordinator's dependency web before anything needs it
there. When panes arrive, the host moves to wherever the view actually reads
it, and that is a decision made with the rendering constraints in hand rather
than guessed at now.

**Alternative considered.** An `App::plugins` field from the start, so later
changes do not have to move it. Rejected: it buys nothing today and pre-commits
the hardest open question in v2 (how a plugin's output reaches the frame) to
whatever shape happened to be convenient for a host that renders nothing.

### D2: Startup runs on a worker; the host is handed back over a channel

**Decision.** `main` spawns one thread that runs discovery and `start_all`,
then sends the finished `PluginHost` back over a channel. `main` does not wait
for it before entering the render loop; it collects the host at shutdown.

**Why.** `start_all` is a sequence of blocking round-trips into plugin VMs, and
a plugin's `init` can run for its whole interrupt budget. Doing that on the UI
thread would put an arbitrary plugin's worst case directly in front of the
first frame. Moving it off means the "does not delay the first frame"
requirement is structural: the UI thread never calls into a plugin at all.

This works because `PluginHost` is `Send` — its `Slot`s hold only channel
senders, join handles, and plain data, never a `Lua`. Each VM stays pinned to
its own thread, exactly as the foundation's design requires. A compile-time
assertion pins that property so a future field that is not `Send` fails the
build rather than silently forcing a redesign.

**Trade-off.** Between boot and the worker finishing, the host does not exist
yet. That is invisible today (nothing reads it) and becomes a real question —
what a pane shows before its plugin is ready — only when panes exist. Recorded
rather than solved.

**Alternative considered.** Starting plugins synchronously before the first
frame, so the host is always present. Rejected: it makes first-frame latency a
function of installed plugin code, which is the failure mode the demand-driven
loop work was spent avoiding.

### D3: Shutdown collects the host with a bounded wait

**Decision.** At shutdown `main` waits for the startup worker with a short
timeout. If the host arrives, it is stopped normally. If it does not, the wait
is abandoned and the process exits.

**Why.** Stopping plugins cleanly is worth a bounded wait — it releases VMs and
gives each plugin its stop path. But a plugin wedged in `init` would otherwise
hold the whole process open, which is exactly the failure the foundation's
shutdown budget already refuses to accept per plugin. Applying the same rule to
the collection step keeps "a wedged plugin never prevents exit" true end to
end. The plugin threads are detached rather than joined in that case; they die
with the process.

### D4: `list` and `status` start plugins; `doctor` does not

**Decision.** The two verbs that report lifecycle state run the full start
sequence in the CLI process. `doctor` runs discovery only.

**Why.** A compile error and an `init` error are the failures users actually
hit, and neither is visible without running the plugin. A `plugin status` that
reported "discovered" for a plugin whose `init` throws would be answering a
different question than the one being asked. `doctor`, by contrast, reports
what discovery *rejected* — plugins that never reach a VM by definition — so
starting anything would be pure cost.

**Trade-off.** `plugin list` pays VM startup for every installed plugin, and
runs each plugin's `init` as a side effect of asking what is installed. These
are diagnostic verbs invoked by hand, the cost is bounded by the execution
bounds, and the alternative is a listing that cannot see the failures it exists
to surface.

### D5: The subcommand is `#[cfg]`-ed out, not stubbed

**Decision.** Without the feature, the `plugin` subcommand does not exist in
the parser — no variant, no help entry, no "plugins are not available" error.

**Why.** It matches what the feature gate means everywhere else: stable builds
do not contain the plugin host, rather than containing a disabled one. A stub
that errors would advertise a capability the binary genuinely lacks, and would
make `--help` a poor description of the binary printing it.

### D6: `cli → plugin` is a path-only allowlist entry

**Decision.** `tests/architecture_rules.rs` gains `plugin` to `cli`'s
`allowed_path_only`, not its `allowed`.

**Why.** It is the same rule `cli` already follows for `agent`: a headless
command reaching into a side-effect subsystem must show that at the call site
rather than hiding it behind an import at the top of the file. Starting VMs
from a short-lived CLI process is exactly the kind of dependency that should be
conspicuous.

## Risks / Trade-offs

- **`plugin list` runs arbitrary plugin code.** Asking what is installed
  executes every installed `init`. → Bounded by the same interrupt and memory
  limits the TUI uses, and contained per VM. It is the price of a listing that
  reports real states; `doctor` remains the side-effect-free view.

- **The startup worker's failure is silent if nothing collects it.** A panic in
  the worker would leave the channel closed and the host absent. → The
  collection path treats an absent host as "nothing to stop" rather than an
  error, and plugin failures are logged by the worker itself, so the diagnostic
  does not depend on the handoff succeeding.

- **First-frame measurement is now sensitive to plugin count.** The bound is
  specified for the no-plugins case, but a user with slow plugins will see boot
  cost elsewhere. → Discovery is the only work on the boot path proper; VM
  startup is off-thread. What a pane does while its plugin is still starting is
  the open question D2 records.

## Migration Plan

Additive and feature-gated. No config file, no schema, no persisted state, and
no existing behavior changes. Rolling back removes the boot hook and the
subcommand; nothing outlives it.

## Open Questions

- **What should a pane show before its plugin has started?** D2's asynchronous
  startup creates a window where a plugin is not yet running. Deferrable: it
  cannot be answered without the pane model, and answering it now would be
  guessing at a rendering contract that does not exist.
