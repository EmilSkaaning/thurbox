# Design — the command registry and the agent API

## 1. Where the registry lives, and why it is pure

```text
plugin.toml ──► session::plugin_manifest::CommandDecl        (pure data)
                        │
                        ▼
        plugin::commands::registry_for(discovered)           (feature-gated adapter)
                        │
                        ▼
        session::plugin_command::CommandRegistry             (pure; ids, schemas, policy)
                        │
        ┌───────────────┴────────────────┐
        ▼                                ▼
  Handler::PaneVisibility          Handler::Plugin
  (kernel: persisted metadata)     (service half: one VM call)
```

The registry is built from **manifests**, not from running plugins. That is the
same rule discovery already follows and it is what makes `command list` answer
for a plugin that has never started, faulted at `init`, or is a view-only plugin
whose service half does not exist. It also keeps listing free of the failure
modes that make `plugin list` start VMs: a command's id, title, and schema are
manifest facts, whereas "does this plugin work" is not.

`session/plugin_command.rs` mirrors `session/spawn_contribution.rs`: pure data
plus pure logic, so argument binding and schema emission are unit-testable with
no VM, no database, and no filesystem. `plugin/commands.rs` is the thin adapter
that turns `DiscoveredPlugin` into it, exactly as `plugin/spawn.rs` does for
contributions.

## 2. Ids: why no collision check is needed

A command's fully-qualified id is `<plugin>.<local-id>`; a generated pane
command's is `<plugin>.<pane>.<op>`. Three properties make the id space
collision-free by construction rather than by validation:

1. Plugin names are unique — discovery refuses a same-source collision and
   resolves a cross-source one, so no two loadable plugins share a prefix.
2. Every manifest identifier is `[a-z][a-z0-9-]*` (`validate_identifier`). **A
   dot cannot appear in a declared id**, so a declared command can never spell
   `<pane>.<op>` and shadow a generated one.
3. Ids are unique *within* a manifest already (`check_ids`), per kind.

So the registry needs no tiebreak, and adding one would be dead code defending
against a state the alphabet forbids. The tests assert the property rather than
the check: a plugin declaring a pane `board` and a command `board` yields four
distinct ids (`p.board`, `p.board.toggle`, `p.board.show`, `p.board.hide`).

## 3. Arguments: typed, not stringly

```toml
[[commands]]
id = "note"
title = "Attach a note"
description = "Attach a note to the calling session"
agent_callable = true
agent_policy = "allow"

[[commands.args]]
name = "text"
type = "string"
required = true

[[commands.args]]
name = "session"
type = "string"
default_from = "session"
```

Three types only — `string`, `integer`, `boolean`. Not because more would be
hard to parse, but because each additional type is a validation rule, a JSON
Schema mapping, a CLI coercion, and a Lua conversion that has to agree in four
places forever. An array or an object argument also removes the reason flags
exist: `--json` already carries any shape a caller wants to pass, and a command
that needs structure can take a string and parse it. The three scalars are what
a flag can express unambiguously.

`default_from` is the mechanism behind `FEATURES-Agent-API.md` §4. It is
restricted to `string` arguments because the identity it fills is an id, and it
is a **manifest error** on any other type — declaring `default_from` on an
integer would be a declaration the host silently ignores.

Binding order matters and is fixed: explicit value → identity default →
required check. So an agent may always override what its environment would have
supplied, and a command whose only source for a required argument is an identity
default fails cleanly with `E_ARGS` when invoked from outside a session rather
than binding an empty string.

### Flag parsing needs the spec

`--name value`, `--name=value`, and a bare `--name` are all valid shapes, but
whether a bare `--name` means `true` or is a missing value depends on the
declared type. So parsing lives on `CommandSpec` (`parse_flags`) rather than in
clap or in the CLI layer: a bare flag is `true` only for a `boolean` argument
and is `E_ARGS` ("missing value") otherwise. That keeps `--text --other x` from
silently binding `text = "true"`.

## 4. Two handler kinds, and why the pane one is kernel-side

`Handler::Plugin` dispatches into the plugin's **service** half. This follows
the rule CLI verbs already established and `FEATURES-Agent-API.md` §6 states
outright: a command an agent should be able to drive must work with the TUI
closed, and the view half has no host without one. A plugin declaring a command
with no service half therefore fails at dispatch with `E_PLUGIN_UNAVAILABLE`
naming the missing half — a runtime error rather than a manifest one, matching
the verb rule, since the same manifest is valid the moment a `service.luau` is
added.

`Handler::PaneVisibility` is handled by the **kernel**, and could not be
anything else: ADR-V21 makes visibility kernel state precisely because a
suspended plugin cannot show its own pane. The kernel's implementation is a
write to the per-pane `metadata` row the TUI already reads
(`get/set_plugin_pane_visible`), which gives three properties for free:

- it works headlessly, with no plugin loaded at all;
- `toggle` is well-defined for a pane that has never been stored, because the
  handler carries the manifest's `default_visible` seed as the value to flip;
- a running TUI sees it without a new channel.

That last point needs one small addition. `PluginUiEvent::Panes` — the only
place stored visibility was read — is emitted on startup and on hot reload, so
an external write would not have been noticed until a reload. So `App` re-reads
stored visibility from `poll_external_changes`, which already runs only when
`PRAGMA data_version` moves, and marks the UI dirty **only** if a pane's flag
actually changed. An idle TUI with no external writes therefore pays one extra
read per detected database change and zero repaints, which is the demand-driven
rule intact.

## 5. Returning a value

A command returns a JSON value, converted from whatever the Lua function
returned. The conversion (`plugin::commands::to_json`) is bounded on depth and
on total node count, for the same reason the view-tree conversion is: it runs on
the plugin's own thread inside the interrupt budget, but a deeply-nested table
would otherwise be turned into a deeply-nested `serde_json::Value` whose *drop*
is recursive on the host side.

Lua's one table type is ambiguous, so the rule is stated rather than guessed: a
table with a `1` key converts as an array (dense prefix only), anything else as
an object with stringified keys, and an empty table as an object. A function,
userdata, or thread is a conversion error naming its type rather than silently
becoming `null` — a plugin returning a closure has a bug, and hiding it costs
the author the error message.

## 6. What `E_DENIED` gates

Two gates, per `FEATURES-Agent-API.md` §5, and only the second is new. The first
is the capability set, which the host already enforces by *absence*: a command
whose plugin was not granted a binding simply cannot call it, with no per-command
check to forget.

The second is the caller gate, and "who is the caller" is exactly the identity
mechanism that already exists: an invocation is **from inside a session** iff
`THURBOX_SESSION` is set in the environment. A command with
`agent_callable = false` or `agent_policy = "deny"` invoked from inside a session
is `E_DENIED`; the same command from a user's shell runs. This is deliberately
not an authentication claim — an agent can unset an environment variable — and
the design says so: policy is per command, not per agent, and the gate exists to
keep an agent from *stumbling* into a user-only command, not to contain a hostile
one. Containment is the capability set.

## 7. What the deferred pieces need

Recorded so the next change starts from a decision rather than a rediscovery.

**The command palette** is a surface, and ADR-V1 makes it an `overlay`-slot
plugin pane. The kernel hosts one plugin pane in one right-column slot
(`docs/PHASE4-PANE-READINESS.md` §5), so there is nowhere to put it. It also
wants fuzzy matching over the registry, which `ui::fuzzy` already provides.

**Plugin keys in F1** need three things, none of them here:

1. `KeyBindings` to resolve a chord to *either* an `Action` or a command id.
   Every lookup, the JSON round-trip, and `Action::rebindable_in_order` are
   keyed on the closed enum today.
2. An open `KeyContext`, since `FEATURES-Keybindings.md` §2 gives every pane
   `pane:<id>`. The overlap predicate is unchanged; the enum is not.
3. The asymmetric conflict rule: a user's rebind steals a chord, a plugin's
   manifest default is **dropped** on collision and reported. v1 has only the
   stealing half.

Once those exist, `[[keybindings]]` widens from its current `{ id, chord }` to
`{ command, key, context }` and the F1 editor renders the merged registry with no
plugin code — which is the whole point of ADR-V21's second carve-out.

## 8. Rejected alternatives

- **Registering commands from a running VM** (`ctx.registerCommand(…)`). Listing
  would then require starting every plugin, which is what `plugin doctor`
  deliberately refuses to do, and a suspended plugin's commands would vanish from
  the palette — the opposite of ADR-V15's intent.
- **One `run(command, args)` dispatcher on the module**, mirroring the CLI verb
  hook. A `commands` table keyed by id is type-checkable in `thurbox.d.luau`,
  makes "declared but not implemented" a precise error, and does not force every
  plugin to write a dispatch `if` ladder.
- **Making a command without a service half a manifest error.** Consistent with
  `PaneWithoutRender`, but wrong here: it would refuse a manifest that is valid
  the instant a `service.luau` appears next to it, and it contradicts
  `FEATURES-Agent-API.md` §6, which specifies `E_PLUGIN_UNAVAILABLE`.
- **Free-form string arguments only**, deferring types. It is the schema that
  makes a command list usable as an agent toolset (ADR-V10); without it the agent
  API is a shell-out convention, which v1 already had.
- **Letting the pane commands go through the TUI** (a channel from the CLI into a
  running App). It would not work headlessly, it needs the control socket this
  change explicitly defers, and it would give one boolean two writers.
