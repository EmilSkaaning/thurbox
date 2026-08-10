# A typed command registry, callable by an agent inside a session

## Why

`[[commands]]` has been in the manifest since the plugin host landed. It is
parsed, its ids are validated, duplicates are refused — and then nothing reads
it. A plugin can declare a command today and no surface anywhere will run it.

That dead declaration is load-bearing for three separate things
(`docs/v2/ARCHITECTURE.md` ADR-V10): keybindings resolve to commands, the
palette lists commands, and an agent inside a session invokes commands. All
three are blocked on the same missing piece, and one of them is already a
recorded debt: ADR-V21 says the kernel generates
`<plugin>.<pane>.{toggle,show,hide}` from the manifest, but the pane-visibility
change shipped a single `TogglePluginPane` action instead, because there was no
registry to generate into. A build with two plugin panes therefore has no way to
reach the second one.

Meanwhile the identity half is already built: thurbox injects `THURBOX_SESSION`
and `THURBOX_TASK` into every spawned agent, and `cli::identity` already resolves
a caller from them. So "an agent operates thurbox in terms of what it wants, not
ids it had to scrape" (`FEATURES-Agent-API.md` §4) needs no new identity
mechanism — only somewhere to spend the one that exists.

## What Changes

- **`[[commands]]` becomes typed.** A command declares a description, typed
  arguments (`[[commands.args]]`), whether an agent may call it, and a caller
  policy. The argument list emits **JSON Schema**, which is the shape agent CLIs
  already consume as tool definitions, so a command list is a toolset with no
  translation layer.
- **A registry is built from manifests with no VM.** Every command is
  `<plugin>.<local-id>`, resolvable for a plugin that has never started —
  discovery is a filesystem walk, and starting a VM to find out what a plugin
  offers would make listing cost as much as running.
- **The kernel generates a pane's visibility commands.** Every declared pane
  gets `<plugin>.<pane>.{toggle,show,hide}` with no plugin code, paying ADR-V21's
  debt. They are handled by the kernel against the same persisted per-pane
  visibility the TUI already stores, so they work with the TUI closed and a
  running TUI picks the change up on its ordinary external-change poll.
- **`thurbox-cli command list|describe|run`.** `list`/`describe` read the
  registry; `run` validates arguments against the declared schema, fills
  identity-defaulted arguments from the caller's injected environment, and
  dispatches — to the kernel for a pane command, to the plugin's **service** half
  for a plugin command (the same reasoning CLI verbs already use: a command an
  agent should be able to drive must work with no TUI).
- **Failures are structured.** `E_UNKNOWN_COMMAND`, `E_ARGS`, `E_DENIED`,
  `E_PLUGIN_UNAVAILABLE` on stdout as JSON with a non-zero exit, so a caller
  branches on a code rather than on prose.

## Capabilities

### New Capabilities

- `plugin-host/commands`: what a command is, how its id is formed, how arguments
  are typed and validated, which commands the kernel generates, and who may
  invoke one.
- `plugin-host/agent-api`: the `thurbox-cli command` surface — discovery,
  invocation, identity defaults, and the error codes.

### Modified Capabilities

- `plugin-host/manifest`: the widened `[[commands]]` table, `[[commands.args]]`,
  and `command` joining the reserved CLI verbs.
- `plugin-host/pane-visibility`: visibility is reachable as a command, and an
  external change to it is picked up by a running TUI.

## Non-goals

- **No control socket.** `FEATURES-Agent-API.md` §6 puts the registry behind a
  JSON-RPC socket for lower-latency callers. It is the only wire protocol v2 has
  and it needs its own hardening (`SECURITY.md` §5: mode `0600` in a `0700`
  directory, peer-UID check, a Windows DACL, unlinked on the panic path) —
  shipping the surface before the hardening would expose `command/run` to any
  local process. Process-per-call over the existing direct-database path is what
  every other `thurbox-cli` verb already does and it is sufficient for an agent
  calling a command per turn.

- **No `agent_policy = "confirm"`.** The design's middle policy queues a prompt
  in the TUI and blocks the CLI call until a human answers. That needs a
  cross-process request/answer channel plus a modal, and until it exists a
  `confirm` declaration would either run unprompted (dangerously wrong) or
  always fail (uselessly wrong). So the vocabulary here is `allow | deny`, and
  `confirm` is a manifest error naming the policies that exist — the same
  treatment an unimplemented motion kind gets. Nothing that ships needs the
  conservative-default rule yet: every command the kernel itself generates is
  non-destructive and reversible.

- **No loop guard.** §5's depth counter stops a command that spawns a session
  whose agent invokes the same command. No plugin can spawn a session: there is
  no `sessions` capability, no binding that reaches the spawn path, and a service
  half has no shell. The counter would have nothing to increment, and a
  mechanism that guards nothing is a mechanism nobody maintains correctly. It
  arrives with the first command that can spawn.

- **No command palette, and no plugin keys in F1.** Both consume this registry
  and neither fits in it. The palette is a surface (ADR-V1 says an overlay-slot
  pane, which needs the multi-pane kernel the Phase 4 audit found missing). F1
  needs `KeyBindings` to key on command ids as well as the closed `Action` enum,
  and its conflict rule to distinguish a user's rebind from a plugin's dropped
  manifest default (`FEATURES-Keybindings.md` §3) — a keymap change, not a
  registry change. `design.md` §7 records what each needs.

- **No `returns` schema.** §2 shows one alongside `args`. Nothing would validate
  it: a command's result is converted from Lua and handed back as JSON, so a
  declared return shape would be a promise the host does not keep.

## Impact

`session/plugin_manifest.rs` (the widened command table and its validation),
a new `session/plugin_command.rs` (the registry, argument binding, JSON Schema —
pure), a new `src/plugin/commands.rs` (manifests → registry, and the bounded
Lua→JSON conversion), `plugin/runtime.rs` + `plugin/service.rs` (one more call
shape), a new `src/cli/commands.rs` plus its `cli/mod.rs` subcommand, and
`app/mod.rs` (applying an externally-changed pane visibility).

No schema change: pane visibility already persists in `metadata`, and the
registry is derived from manifests on every run.
