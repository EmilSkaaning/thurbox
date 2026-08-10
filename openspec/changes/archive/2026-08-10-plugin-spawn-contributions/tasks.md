# Tasks — spawn contributions

## 1. Manifest surface

- [x] 1.1 Add `Capability::Spawn` to `src/session/plugin_manifest.rs`.
- [x] 1.2 Add `SpawnDecl { env }` and `PluginManifest::spawn`, `deny_unknown_fields`.
- [x] 1.3 Validation: a `[spawn]` table without the `spawn` capability is `ManifestErrorKind::SpawnWithoutCapability`.
- [x] 1.4 Tests: declared+granted validates, declared-without-capability rejected, capability alone validates, unknown key rejected.

**Verify:** `cargo nextest run -E 'test(plugin_manifest)'`

## 2. Policy: reserved keys

- [x] 2.1 Add `Rejection::Reserved` and `resolve_over(reserved, …)`; `resolve` delegates with an empty set.
- [x] 2.2 Tests: a reserved key is refused and named; an unreserved key still lands.

**Verify:** `cargo nextest run -E 'test(spawn_contribution)'`

## 3. Process registry

- [x] 3.1 Add a process-wide registry to `session::spawn_contribution` — `Registry`, `publish`, `published` — ordered by plugin name so conflicts resolve identically on every run.
- [x] 3.2 Test: re-publish replaces, deterministic order, empty by default.

**Verify:** `cargo nextest run -E 'test(spawn_contribution)'`

## 4. Enforcement point

- [x] 4.1 Apply the registry in `session_ops::inject_thurbox_env`, after the kernel vars, passing them as reserved.
- [x] 4.2 Establish by measurement that tmux cannot deliver a contributed `PATH`, and give `path` no manifest surface (design §5).
- [x] 4.3 `tracing::warn!` every rejection.
- [x] 4.4 Tests: variable lands, kernel var survives, denied key absent, a remote session still takes a plain variable, empty registry leaves env byte-identical.

**Verify:** `cargo nextest run -E 'test(session_ops)'`

## 5. Publishing from discovery

- [x] 5.1 Add `plugin::spawn::registry_for` + `publish_from_discovery` (feature-gated) and call it from both binaries after discovery.
- [x] 5.2 Tests: only plugins with a `[spawn]` table and the capability are published; nothing published carries a `PATH` prepend.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 6. Surfacing

- [x] 6.1 Add a spawn-contribution section to `thurbox-cli plugin doctor`, derived from discovery only.
- [x] 6.2 Tests: accepted and refused entries listed with reasons; no plugin code executed; clean when nothing is declared.

**Verify:** `cargo nextest run -E 'test(plugins)' --features plugins`

## 7. Documentation

- [x] 7.1 Document the `[spawn]` table and the policy in `CLAUDE.md`'s plugin section.

**Verify:** `rumdl check .`
