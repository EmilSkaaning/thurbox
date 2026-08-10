## Context

See `proposal.md` — Why. Two facts constrain the input half:

- **Plugin calls are round-trips to another thread.** Rendering tolerates that
  because it is asynchronous and cached. A keypress cannot be: thurbox must
  decide *now* whether to also act on it.
- **`InputFocus` is a closed enum** the focus ring walks, and a plugin pane has
  to become a stop in it.

## Goals / Non-Goals

**Goals:** a pane that responds to keys without ever letting a plugin freeze or
trap the user; a reload loop that is genuinely a save away.

**Non-Goals:** mouse, global chords, partial reload.

## Decisions

### D1: A key is a bounded, synchronous round-trip

**Decision.** When a plugin pane is focused, the key is sent to the plugin's
thread and the UI thread waits for the answer with a **short timeout**. On
timeout the key is treated as unconsumed and the plugin is recorded as failed.

**Why.** "Did the plugin consume this key?" has to be answered before thurbox
decides what to do with it, and deferring the decision would mean either
double-handling or dropping keys. Waiting is therefore the honest design — but
it is the one place the UI thread touches a plugin, so it is bounded twice
over: by the interrupt budget inside the VM and by a timeout outside it.

The timeout is short enough that a wedged plugin costs one visibly-dropped
frame rather than a hang, and a plugin that trips it is marked failed, so the
cost is paid once rather than on every keystroke.

**Alternative considered.** Fire-and-forget with an "unconsumed" default.
Rejected: a plugin could then never reliably consume a key, which makes the
capability pointless — the pane could display but never act.

### D2: Unconsumed keys always fall through

**Decision.** A key the plugin does not consume is handled by thurbox as
normal, and the focus/quit chords are never offered to the plugin at all.

**Why.** A pane that could swallow every key would let a third-party plugin
trap a user in it. Keeping the escape routes kernel-only is the same reasoning
that keeps the F1 editor kernel chrome in ADR-V21.

### D3: Reload reuses `PluginHost::reset`

**Decision.** Reload is `reset` (stop, return to `discovered`) followed by the
normal start path.

**Why.** The lifecycle was written to admit exactly this, with a test asserting
a reset plugin starts with a fresh VM. Reusing it means reload cannot acquire
its own subtly different semantics, and "no state survives" is already proven.

### D4: Source watching is mtime polling on the existing render cycle

**Decision.** The render worker records each loaded plugin's entry-file mtime
and reloads when it moves.

**Why.** There is already a worker waking on a fixed cadence with the host in
hand; a filesystem-notification dependency would buy sub-second latency for a
loop whose bottleneck is the human typing. Polling also degrades predictably on
network filesystems, where notification APIs are least reliable.

## Risks / Trade-offs

- **The UI thread waits on a plugin for the timeout.** → Bounded, once per
  offending plugin (it is then failed), and only for panes the user focused.
- **mtime granularity can miss two saves in the same second.** → The next cycle
  catches it; the cost is one second of staleness in a dev loop.
- **A reload drops in-flight renders.** → They are recomputed on the next
  cycle; a stale tree is already the specified behaviour while rendering.

## Migration Plan

Additive and feature-gated.

## Open Questions

None blocking.
