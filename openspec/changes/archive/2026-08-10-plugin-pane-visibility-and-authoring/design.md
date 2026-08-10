## Context

See `proposal.md` — Why. `docs/v2/` ADR-V21 settles the ownership question:
pane visibility is kernel state, seeded by the manifest and persisted per pane.

Two existing facts shape the rest:

- **`Action` is a closed Rust enum** with a fixed `rebindable_in_order()`. A
  generated command per pane cannot be an `Action` variant, because panes are
  discovered at runtime.
- **`metadata` is thurbox's existing small-keyed-value table**, already used for
  the active theme and the pending focus request.

## Goals / Non-Goals

**Goals:**

- Give a plugin pane the same show/hide model every native panel has, without
  each plugin inventing one.
- Make a malformed view node an authoring-time error rather than a runtime one.
- Get a Luau linter into CI before more `.luau` ships.

**Non-Goals:**

- The per-pane command space. That needs the command registry.
- Input into plugins.

## Decisions

### D1: One kernel toggle now, the per-pane command space with the registry

**Decision.** Add a single rebindable `Action::TogglePluginPane` that toggles
the plugin pane. Do **not** synthesize `<plugin>.<pane>.toggle` commands yet.

**Why.** ADR-V21's generated commands presuppose a command registry that does
not exist until Phase 5. Faking one now would mean inventing a second,
throwaway namespace for dynamic commands and then migrating every plugin off
it. A single action gets the uniform show/hide behaviour that ADR-V21 is
actually protecting — panes are toggleable, and rebindably so — while leaving
the registry's design free.

**Trade-off, stated plainly.** With several plugin panes this toggles the first
one, which is not the end state. It is honest about being an interim step
rather than pretending the registry exists.

### D2: Visibility lives in `metadata`, keyed by `<plugin>.<pane>`

**Decision.** Persist one row per pane, keyed `plugin_pane_visible.<plugin>.<pane>`,
in the existing `metadata` table. Absent means "use the manifest's seed".

**Why.** It is a small keyed value that must outlive the process and sync
between instances — exactly what `metadata` already does for the theme, and
what its `PRAGMA data_version` polling already propagates. A dedicated table
would add a migration for a key/value pair.

Keying on `<plugin>.<pane>` rather than an index means a user's choice survives
installing another plugin, which an index would not.

### D3: `ui` constructors are Rust functions in the host module

**Decision.** Build the `ui` table in Rust alongside the capability bindings,
ungated.

**Why.** They construct plain tables and grant no host power, so gating them
behind a capability would be theatre. Implementing them in Rust rather than as
a shipped `.luau` file keeps them inside the sandbox's frozen module table —
a plugin cannot replace `ui.text` for itself and then be surprised by what the
host converts.

**Alternative considered.** Shipping them as a Luau source module the plugin
requires. Rejected: it would live in the plugin's own directory space, and the
frozen-table guarantee would not extend to it.

### D4: `luau-analyze` runs over the bundled plugin in CI

**Decision.** Ship a `thurbox.d.luau` declaring the `@thurbox` surface and add
a CI job running `luau-analyze` in strict mode over `src/plugin/bundled/`.

**Why.** `docs/v2/` Phase 0 asks for the Luau toolchain *before* the first Luau
PR precisely so that PR does not carry it. That ordering was missed — Luau is
already shipping, checked by nothing. Adding it now stops the gap widening, and
the bundled plugin is the natural subject: it is the worked example, so an
example that does not type-check teaches the wrong thing.

## Risks / Trade-offs

- **One toggle for N panes** → D1's stated interim; the registry supersedes it.
- **A stale visibility row for an uninstalled plugin** lingers in `metadata`. →
  Harmless (it is consulted only when that pane exists again) and self-correcting
  if the plugin is reinstalled. Pruning belongs with an uninstall verb.
- **`luau-analyze` availability** varies by machine. → CI installs it; the
  local `just` target skips with a notice rather than failing a developer who
  does not have it.

## Migration Plan

Additive and feature-gated. The one persisted key is new, absent by default,
and read only when a plugin pane exists.

## Open Questions

None that would change this design. The per-pane command space is deferred by
decision, not by uncertainty.
