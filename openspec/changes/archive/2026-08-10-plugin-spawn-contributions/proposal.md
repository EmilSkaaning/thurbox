# Wire spawn contributions into the spawn path

## Why

v1 extensions could patch an agent's argv through `[[agent_patches]]`, and the
built-in hooks extension depends on exactly that. If plugins cannot reach a
spawned session at all, the extensions we intend to replace with plugins get
strictly worse — that is
`docs/v2/LIMITATIONS.md` §4.5, and `FEATURES-Backend-API.md` §11 restores a
bounded form of it.

The dangerous half of that feature already landed: `session::spawn_contribution`
holds the denylist, the `PATH` confinement rule and the append-only conflict
rule, fully unit-tested. Nothing calls it. So today the policy is a document
with tests, not an enforcement point, and a plugin has no way to contribute at
all — the *safe* half is missing, which means the unsafe half is the one that
gets built next in a hurry.

## What Changes

- **A plugin declares its contribution in its manifest**, not in Luau. A
  `[spawn.env]` table names environment variables; the kernel reads it during
  discovery, with no VM involved.
- **`spawn` becomes a declared capability.** A `[spawn]` table without the
  `spawn` capability is a manifest error, so the reach stays reviewable from the
  capability list alone.
- **The spawn path applies the policy.** Every session thurbox launches —
  headless, TUI, restart, restore — resolves contributions through
  `spawn_contribution::resolve` before its environment is finalized.
- **The kernel's own environment is reserved.** `THURBOX_SESSION` and its
  siblings are the identity a session proves itself with; a plugin cannot
  overwrite them, and trying is a recorded rejection rather than a no-op.
- **Rejections are surfaced twice**: a warning in `thurbox.log` at the moment of
  the spawn, and a `thurbox-cli plugin doctor` section that re-derives every
  verdict from the manifests — so "why is my variable not there" is answerable
  without a spawn or a log.

## Capabilities

### New Capabilities

- `plugin-host/spawn-contributions`: what a plugin may add to a spawned agent's
  environment, what the kernel refuses, and how a refusal becomes visible.

### Modified Capabilities

- `plugin-host/manifest`: the `[spawn]` table and its validation.
- `plugin-host/capabilities`: `spawn` as a declared capability.

## Non-goals

- **No `PATH` prepends.** `SECURITY.md` §8 allows a plugin to prepend
  directories under `{plugin_data}`, and the policy layer already implements the
  confinement rule. There is no manifest surface for it, because the session
  backend cannot deliver one: tmux replaces a pane's `PATH` with the server's
  own and ignores both `new-window -e PATH=…` and `set-environment PATH`
  (verified against tmux 3.5a). Honouring a prepend means folding it into the
  launched argv, which is the same seam argument contributions need. Shipping
  the field anyway would have been a declaration that silently does nothing —
  the exact failure this change's rejection machinery exists to prevent.

- **No `contribute(ctx)` callback.** `FEATURES-Backend-API.md` §11 describes a
  per-spawn Luau function with a 500 ms fail-open deadline. The seam that would
  need — a spawn-environment step guaranteed to run off the render loop — does
  not exist yet: `App::build_spawn_inputs` finalizes the environment on the UI
  thread. A static manifest declaration needs no VM at all, so it is both the
  safe subset and the one that can ship now. `design.md` records what the
  dynamic form would additionally require.
- **No argument contributions.** §11 also allows appending argv. Args are
  rewritten per host by `session_ops::spawn::adapt_def_for_launch`; contributing
  into that is a separate contract with its own remote-rewrite question.
- **No `hook_schema` contribution.** It rides on the argv half.

## Impact

`session/plugin_manifest.rs` (the `[spawn.env]` table, the `spawn` capability,
one validation rule), `session/spawn_contribution.rs` (the reserved-key rule and
the process registry), `session_ops/mod.rs` (the enforcement point),
`plugin/spawn.rs` (publishing manifests into the registry), the two binaries
(installing it), and `cli/plugins.rs` (the doctor section).

No schema change: nothing here is persisted.
