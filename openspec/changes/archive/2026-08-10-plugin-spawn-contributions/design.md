# Design — spawn contributions

## 1. Why a manifest declaration rather than a Luau callback

`FEATURES-Backend-API.md` §11 sketches:

```lua
spawn = { contribute = function(ctx, spawn) ... end }
```

A per-spawn callback needs a place to run. The only place the spawn environment
is finalized today is `session_ops::inject_thurbox_env`, and one of its five
callers — `App::build_spawn_inputs` — runs on the UI thread. Entering a Luau VM
there would put plugin code on the render loop, which
`docs/PERFORMANCE.md` and `docs/CONSTITUTION.md` both forbid, and the 500 ms
fail-open deadline §11 specifies is a *frame budget* violation of two orders of
magnitude even when it is honoured.

A static declaration removes the question entirely:

- The contribution is known at **discovery** time, so the spawn path reads a
  snapshot rather than calling anything.
- `thurbox-cli plugin doctor` can report the exact verdict without starting a
  VM, which is what keeps its existing "doctor must not execute plugin code"
  guarantee intact.
- It is the subset that is actually equivalent to v1's `[[agent_patches]]`,
  which was also static data in a TOML file.

The dynamic form stays possible later. What it needs first is a spawn-env step
that is guaranteed to be off the UI thread — i.e. moving contribution
resolution out of `build_spawn_inputs` and into the spawn worker that
ADR-P12 already introduced. That is a change to the TUI spawn flow, not to this
contract, and the manifest declaration is forward-compatible with it: a plugin
that later gains `contribute` simply produces the same `SpawnContribution`
shape from a different source.

## 2. Where the policy runs

```text
discovery  ──►  plugin::spawn::publish_from_discovery()
                        │  (feature = "plugins", called once by each binary)
                        ▼
        session::spawn_contribution::REGISTRY   (process-wide, RwLock)
                        │
                        ▼
   session_ops::inject_thurbox_env  ──► resolve_over(reserved, …) ──► config.env
                                                  │
                                                  └─► tracing::warn! per rejection
```

`inject_thurbox_env` is the enforcement point because it is the single function
every spawn path already funnels through — headless spawn, headless restart, the
TUI's `build_spawn_inputs`, `Ctrl+R` restart, and session restore. Applying the
policy anywhere else would mean five call sites that can each be forgotten
independently.

The registry is read-only from the spawn path and is written once per process
after discovery. It is empty in a build without the `plugins` feature, and the
`if registry.is_empty() { return }` fast path means a stable binary does
exactly what v1 does — the property the acceptance snapshots already pin.

## 3. Module boundaries

No new architecture edge. The registry lives in `session::spawn_contribution`
(pure data), which `session_ops` already reaches and `plugin` already reaches.
The publisher is `plugin::spawn`, and the two crate roots (`main.rs`,
`bin/thurbox-cli.rs`) — both exempt from `tests/architecture_rules.rs` — call
it. That is the same shape the plugin host already uses to stay out of `app`.

The alternative, letting `session_ops` reach `plugin` directly, was rejected:
it would put `discover()`'s filesystem walk on the spawn path and make a
headless `session create` pay for plugin discovery whether or not any plugin
contributes.

## 4. Reserved keys

`resolve_over` takes the set of keys the caller has already claimed. The spawn
path passes every key `inject_thurbox_env` has just written — the `THURBOX_*`
identity vars and the config/data dir overrides. A plugin overwriting
`THURBOX_SESSION` would not merely confuse a session; it would make
`thurbox-cli message send` inside that session act as a *different* session,
which is an authorization bug, not a configuration one.

Ordering matters and is deliberate: the kernel writes first, so "reserved" is
mechanical rather than a hardcoded name list that could drift from what
`inject_thurbox_env` actually sets.

## 5. `PATH`, and why it has no manifest surface

`SECURITY.md` §8 permits `PATH` prepends confined to `{plugin_data}`, and
`session::spawn_contribution` already implements that rule. It is not exposed,
because the session backend cannot carry it.

Measured against tmux 3.5a, a pane's `PATH` is whatever the tmux *server*
inherited; neither `new-window -e PATH=…` nor `set-environment PATH` reaches
the pane, while any other variable passed the same way arrives intact. The
first implementation of this change did wire prepends through, and an
end-to-end spawn showed the contributed `PATH` silently absent from the agent's
environment while `CI_TOKEN_FILE` arrived — the exact "looks installed, does
nothing" failure the rejection machinery exists to prevent, reintroduced by the
feature meant to prevent it.

Delivering a prepend means composing it into the launched command
(`sh -c 'PATH=…:$PATH exec …'`), which changes the argv — and argv is already
rewritten per host by `session_ops::spawn::adapt_def_for_launch` and folded into
a single PowerShell token for psmux. That is the same seam argument
contributions need, and it belongs with them.

So `[spawn]` accepts `env` and nothing else: an author who writes `path = […]`
gets a manifest error naming the key, rather than a line that quietly does
nothing. Contributed *environment* does travel to a remote host — it is opaque
data, the same rule `inject_thurbox_env` already applies to the identity vars
versus the local-path vars.

## 6. Failure shape

Nothing here can fail a spawn. Every refusal is a `Rejection` value, appended
to a list, logged, and reported — the session still launches. That is the
fail-open rule from §11, and it is also what keeps a broken plugin from taking
the user's ability to start work with it.
