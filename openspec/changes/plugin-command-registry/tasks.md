# Tasks — the command registry and the agent API

## 1. Manifest surface

- [x] 1.1 Widen `CommandDecl` in `src/session/plugin_manifest.rs`: `description`,
      `agent_callable` (default true), `agent_policy` (`allow`/`deny`, default
      allow), `args: Vec<CommandArgDecl>`.
- [x] 1.2 Add `CommandArgDecl { name, ty, required, description, default_from }`
      with `ArgType` and `IdentityDefault`, both `deny_unknown_fields`.
- [x] 1.3 Validation: argument names are identifiers and unique per command; an
      identity default only on a `string` argument.
- [x] 1.4 Add `command` to `RESERVED_CLI_VERBS`.
- [x] 1.5 Tests: full and minimal declarations; unknown policy / type / identity
      source rejected naming the alternatives; duplicate and malformed argument
      names; identity default on an integer rejected; `command` reserved.

**Verify:** `cargo nextest run -E 'test(plugin_manifest)'`

## 2. The registry (pure)

- [x] 2.1 New `src/session/plugin_command.rs`: `CommandSpec`, `Handler`,
      `VisibilityOp`, `ArgValue`, `BoundArgs`, `CommandRegistry`.
- [x] 2.2 `CommandRegistry::from_manifests` — plugin-qualified declared commands
      plus the three generated visibility commands per pane.
- [x] 2.3 `CommandSpec::args_schema()` emitting JSON Schema.
- [x] 2.4 `CommandSpec::parse_flags` (bare flag is `true` only for a boolean),
      `bind_flags`, `bind_json`, sharing one required/identity-default finish.
- [x] 2.5 `CommandSpec::denial` — the per-invocation agent-callable / policy
      gate — plus `agent_reachable`, the same rule with the caller quantified
      away, for the listing.
- [x] 2.6 Tests: ids namespaced; a declared id cannot collide with a generated
      one; generated commands take no arguments; schema shape for typed and
      empty commands; every binding failure; explicit value beats an identity
      default; a required identity-defaulted argument with no identity fails.

**Verify:** `cargo nextest run -E 'test(plugin_command)'`

## 3. Discovery adapter and the Lua→JSON conversion

- [x] 3.1 New `src/plugin/commands.rs`: `registry_for(discovered)`.
- [x] 3.2 `to_json` with depth and node-count bounds; a sequence becomes an
      array, anything else an object, an unrepresentable value is an error.
- [x] 3.3 Tests: a broken plugin still contributes commands; a pane-only plugin
      yields three commands; array/object/nil conversion; depth and size
      refusals; a function refused naming its type.

**Verify:** `cargo nextest run -E 'test(plugin::commands)' --features plugins`

## 4. Dispatch into the service half

- [x] 4.1 `PluginVm::run_command` — look up `commands[<local-id>]`, call with an
      args table, convert the result.
- [x] 4.2 `Request::Command` + `PluginThread::run_command` +
      `ServiceHost::run_command`.
- [x] 4.3 Tests: a command runs and returns; arguments arrive with their types;
      a declared-but-unimplemented command errors naming it; a runtime failure
      is reported, not swallowed.

**Verify:** `cargo nextest run -E 'test(plugin)' --features plugins`

## 5. The CLI surface

- [x] 5.1 New `src/cli/commands.rs` with `list` / `describe` / `run`; register
      the `command` subcommand in `cli/mod.rs` behind the `plugins` feature.
- [x] 5.2 `run` dispatch: kernel pane-visibility commands against
      `get/set_plugin_pane_visible`; plugin commands via a service host, as the
      verb path does.
- [x] 5.3 Structured failures: `E_UNKNOWN_COMMAND`, `E_ARGS`, `E_DENIED`,
      `E_PLUGIN_UNAVAILABLE`, each with a non-zero exit.
- [x] 5.4 Identity from `THURBOX_SESSION` / `THURBOX_TASK`; a caller inside a
      session is one with `THURBOX_SESSION` set.
- [x] 5.5 Tests: list and scoped list, unknown-plugin scope, describe, every
      error code, flags vs JSON equivalence, a bare non-boolean flag, the pane
      commands' effect on the store including toggle-from-seed.

**Verify:** `cargo nextest run -E 'test(cli::commands)' --features plugins`

## 6. The TUI picks up an external visibility change

- [x] 6.1 `App::apply_stored_plugin_pane_visibility` returning whether anything
      changed; call it from `poll_external_changes` on a detected change.
- [x] 6.2 Tests (acceptance): an external hide takes effect; an unchanged store
      reports no change; a pane with no stored choice is untouched.

**Verify:** `cargo nextest run -E 'test(plugin_pane)' --features plugins`

## 7. Authoring surface and docs

- [x] 7.1 `thurbox.d.luau`: the `Service` shape including `commands`.
- [x] 7.2 `CLAUDE.md`: the command registry, the generated pane commands, and
      `thurbox-cli command`.
- [x] 7.3 `rumdl check .` clean.

**Verify:** `./scripts/dev/lint-luau.sh && rumdl check .`

## 8. Full verification

- [x] 8.1 `cargo fmt --all -- --check`
- [x] 8.2 `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] 8.3 `cargo clippy --all-targets -- -D warnings`
- [x] 8.4 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] 8.5 `cargo nextest run --all` and `--all --features plugins`
- [x] 8.6 `cargo tree --edges normal | grep -c mlua` is 0
- [x] 8.7 `cargo test --test architecture_rules`
