## Why

Every plugin today lives and dies with the TUI. That silently revokes a
guarantee v1 makes: automations fire with the TUI closed, driven by a detached
tmux heartbeat keeper. A plugin that syncs a tracker or watches CI would stop
the moment you quit — so the integrations v1 ships as `Exec` automations cannot
become plugins without becoming worse.

`docs/v2/` ADR-V16 draws the seam: a plugin has a **headless service half** and
a **TUI-only view half**, in separate VMs with separate capability grants,
hosted by whichever of the TUI, the heartbeat keeper, or a `thurbox-cli`
invocation needs it first.

## What Changes

- **A second entry point.** A plugin may ship `service.luau` alongside its view
  entry. It runs in its own VM on its own thread, with its own capability
  grant, and it runs whether or not a TUI exists.
- **The halves fault independently.** A service that crashes degrades the
  plugin's background work; it does not remove its pane, and vice versa.
- **Machine-wide single instance.** An advisory lock in the database means a
  running TUI and a heartbeat tick never both run one plugin's service. The
  lock is taken per plugin, carries its holder, and expires so a killed holder
  cannot wedge a plugin forever.
- **Namespaced plugin storage (`ctx.kv`).** A plugin reads and writes its own
  keys in the existing database. A plugin cannot address another plugin's
  namespace — the namespace is applied by the host, not passed by the plugin.
- **The headless tick hosts services.** `thurbox-cli automation tick`, which
  the keeper already loops every 60 s, starts due plugin services — so a
  service-only plugin keeps working with the TUI closed.

## Capabilities

### New Capabilities

- `plugin-host/service`: the service entry point, its lifecycle, how it is
  hosted, and how it fails independently of the view half.
- `plugin-host/storage`: namespaced key/value storage — what a plugin can
  address, what it cannot, and what persists.
- `plugin-host/single-instance`: the advisory lock — who holds it, what
  happens to a contender, and how a dead holder is recovered.

### Modified Capabilities

- `plugin-host/capabilities`: adds the storage capabilities' service-side
  meaning and the service grant being separate from the view grant.

## Non-goals

- **No schedules, event bus, or cross-half RPC.** Those are the rest of Phase
  2 and each needs its own contract; this change is the hosting substrate they
  sit on.
- **No CLI verb registration.** Next change.
- **No spawn contributions or skills registry.** Later in Phase 2, and the env
  denylist ships with them.
- **No network or shell capability.** A service can compute and store; reaching
  outward is a separate grant with its own threat model.

## Impact

`session/plugin_manifest.rs` (service entry + per-half capabilities),
`plugin/` (a service supervisor beside the existing one), `storage/` (a
`plugin_kv` table and the lock table, both new schema), and
`cli/automations.rs` (hosting from the tick).

This is the first plugin state in the database proper rather than in
`metadata`, so it carries a schema migration.
