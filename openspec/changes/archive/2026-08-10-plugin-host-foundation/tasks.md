## 1. Feature gate and dependency

- [x] 1.1 Add a `[features]` section to `Cargo.toml` with `plugins = ["dep:mlua"]`, empty `default`, and add `mlua = { version = "0.12", features = ["luau", "vendored"], optional = true }`. Do **not** enable `send` (design D1).
- [x] 1.2 Set `rust-version = "1.86"` in `Cargo.toml` and correct `clippy.toml`'s `msrv = "1.75"` to `1.86` (design D6). Note the `plugins` feature's 1.88 requirement in the `[features]` comment.
- [x] 1.3 Regenerate `Cargo.lock` and run `cargo deny check bans licenses sources` with the feature enabled; add any license exceptions the vendored Luau sources need to `deny.toml`.
- [x] 1.4 Verify stable is untouched: `cargo tree | grep -c mlua` returns 0, and `cargo nextest run --all` passes with no feature flags.

**Verify:** `cargo build`, `cargo build --features plugins`, `cargo deny check bans licenses sources`

## 2. Manifest as pure data (`plugin-host/manifest`)

- [x] 2.1 Create `src/session/plugin_manifest.rs` with the manifest types — identity (`name`, `api_version`), provided `panes`/`commands`/`keybindings`, and requested `capabilities` — deriving `serde::Deserialize` with `#[serde(deny_unknown_fields)]` for the unknown-key rejection requirement. Register the module in `src/session/mod.rs`.
- [x] 2.2 Implement name and id validation (lowercase alphanumeric + hyphen, leading letter, ≤64 chars) as a shared pure helper, applied to the plugin name and to every pane/command/keybinding id.
- [x] 2.3 Implement per-kind duplicate-id detection within a manifest, and the API-version compatibility check against a host-declared supported version constant.
- [x] 2.4 Implement the parse entry point returning a structured error that carries the manifest path plus the specific cause (missing field, malformed value, unknown key, duplicate id, TOML syntax, I/O).
- [x] 2.5 Add unit tests covering every scenario in `specs/plugin-host/manifest/spec.md`: minimal valid manifest, each missing required field, malformed names, duplicate ids per kind, same id across kinds (accepted), unknown key, invalid TOML, unreadable file, incompatible and compatible `api_version`.

**Verify:** `cargo nextest run -E 'test(plugin_manifest)'`, `cargo test --test architecture_rules`

## 3. Module boundary

- [x] 3.1 Create `src/plugin/mod.rs` and register it in `src/lib.rs` and `src/main.rs` behind `#[cfg(feature = "plugins")]`.
- [x] 3.2 Add the `ModuleRules` entry for `plugin` to `tests/architecture_rules.rs` with `allowed: &["session", "paths"]` and empty `allowed_path_only`, with a comment stating why `agent`, `git`, `storage`, `ui`, and `app` are excluded (design D3).
- [x] 3.3 Update `CLAUDE.md`'s Module Dependency Rules and `docs/CONSTITUTION.md` §2 to include the new module, per the repo rule that an architecture change updates both in the same change.
- [x] 3.4 Confirm `every_module_is_governed` passes in both feature configurations.

**Verify:** `cargo test --test architecture_rules`, `cargo test --test architecture_rules --features plugins`

## 4. Capability model (`plugin-host/capabilities`)

- [x] 4.1 Define the closed capability vocabulary as an enum in `src/session/plugin_manifest.rs` (pure data, so manifest validation can reject unknown names without the runtime), with a `FromStr` that fails on unrecognized names.
- [x] 4.2 Wire capability validation into manifest parsing so an unknown capability fails at the manifest stage, before any VM exists.
- [x] 4.3 Create `src/plugin/capabilities.rs` holding the granted-set type and the function that builds a plugin's module table from its granted set — bindings for undeclared capabilities are never inserted (design D4).
- [x] 4.4 Add unit tests for the vocabulary and grant-set construction: unknown capability rejected pre-VM, empty declaration yields an empty grant set, granted set is reportable per plugin.

**Verify:** `cargo nextest run -E 'test(capabilit)' --features plugins`

## 5. Runtime (`plugin-host/runtime`)

- [x] 5.1 Create `src/plugin/runtime.rs`. Implement VM construction: build the `Lua` inside the owning thread, install the capability-scoped module table, then call `Lua::sandbox(true)`.
- [x] 5.2 Install the instruction budget via `Lua::set_interrupt` and the memory ceiling via `Lua::set_memory_limit`, sourced from a host-owned bounds struct that no manifest can influence (design D7). Use provisional defaults and mark them as calibration targets in the change's follow-up notes, not in code comments.
- [x] 5.3 Replace `require` with a resolver scoped to the plugin's own directory, rejecting any path that escapes it after canonicalization.
- [x] 5.4 Implement the plugin thread: own the VM, service an `mpsc` request channel with one-shot replies, and terminate the VM on a budget or memory failure rather than resuming it.
- [x] 5.5 Implement fault containment at the host boundary — plugin errors become a failure record carrying plugin id, entry point, and message; no error propagates as a panic into the host.
- [x] 5.6 Add tests for: cross-plugin global isolation, cross-plugin stdlib mutation isolation, budget exceeded, budget respected, memory ceiling exceeded, uncaught error contained, failure then later success, absent filesystem/process stdlib entry points, `require` escaping the plugin directory rejected, `require` within the directory accepted.
- [x] 5.7 Add a test asserting no VM and no thread are created when discovery yields nothing.

**Verify:** `cargo nextest run -E 'test(plugin::runtime)' --features plugins`

## 6. Discovery (`plugin-host/discovery`)

- [x] 6.1 Add a `PathKind` variant and `plugins_directory()` accessor to `src/paths.rs` resolving `~/.config/thurbox/plugins/`, honoring the existing `THURBOX_CONFIG_DIR` override and `set_test_dir` (design D5).
- [x] 6.2 Create `src/plugin/discovery.rs` with the ordered source list — a compile-time bundled registry (empty for now, type and ordering in place) followed by the user directory.
- [x] 6.3 Implement directory scanning: a directory with a root manifest is one plugin; no manifest means skip silently with no recursion; loose files are skipped. Sort results for determinism independent of filesystem order.
- [x] 6.4 Implement collision resolution — later source wins and records the shadowed plugin as overridden; a collision within one source rejects both and reports the conflict.
- [x] 6.5 Implement the discovery outcome type carrying loadable plugins, overridden plugins, and failures with causes and paths.
- [x] 6.6 Add tests using `TestPathGuard`-style tempdirs for: both sources populated, determinism across two runs, manifest present/absent/loose file, missing user directory (not created), unreadable user directory, user shadows bundled, same-source collision, one malformed among several, all malformed.

**Verify:** `cargo nextest run -E 'test(plugin::discovery)' --features plugins`

## 7. Lifecycle (`plugin-host/lifecycle`)

- [x] 7.1 Create `src/plugin/lifecycle.rs` with the state enum (`Discovered`, `Loaded`, `Running`, `Stopped`, `Failed { transition, cause }`) and the transition function enforcing the legal order.
- [x] 7.2 Implement load (compile the entry module) and initialize (call the optional init entry point exactly once with a plugin-scoped context), with a plugin lacking an init entry point reaching `Running` regardless.
- [x] 7.3 Implement the registry that holds every known plugin's state, granted capabilities, and failure cause, and reports each plugin exactly once.
- [x] 7.4 Implement deterministic initialization order derived from discovery order, with no inter-plugin dependency mechanism.
- [x] 7.5 Implement shutdown: stop every running plugin, release VM and thread, abandon a plugin that exceeds the shutdown budget and record it as failed to stop without blocking exit or the remaining stops.
- [x] 7.6 Add tests for: normal progression, compile failure skipping init, init called exactly once, context reflects declared capabilities only, one plugin failing while others run, all plugins failing, stable order across runs, clean shutdown, hung plugin at shutdown, stop-then-load-again yielding a fresh VM with no prior state.

**Verify:** `cargo nextest run -E 'test(plugin::lifecycle)' --features plugins`

## 8. Test fixtures

- [x] 8.1 **Deviation from plan — fixtures are built inline, not checked in.** The plan called for a `tests/fixtures/plugins/` tree. Implemented instead as per-test builders (`plugin_dir` in `src/plugin/runtime.rs`, `write_plugin` in `discovery.rs` and `lifecycle.rs`) that write a `plugin.toml` plus entry file into a fresh `tempfile::tempdir()`. Reason: a shared on-disk tree puts a test's input a directory away from its assertion and lets one test's mutation reach another, while these tests are largely *about* filesystem layout (missing manifest, nested manifest, name collision, escaping `require`) and so need to construct that layout per case anyway.
- [x] 8.2 Every plugin test builds its own tempdir and passes it explicitly via `discover_in(bundled, user_root)`; no test reads the real config directory, and `discover()` (which does) is never called from a test.

**Verify:** `cargo nextest run --features plugins`

## 9. CI and lint

- [x] 9.1 Add a `plugins` job to `.github/workflows/ci.yml` running `cargo build --features plugins`, `cargo nextest run --all --features plugins`, and `cargo clippy --all-targets --features plugins -- -D warnings` on a toolchain ≥ 1.88.
- [x] 9.2 Confirm the default jobs still run without the feature so both configurations are covered.
- [x] 9.3 Run the full lint suite in both configurations and fix findings.

**Verify:** `just lint`, `cargo clippy --all-targets --features plugins -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features plugins`

## 10. Close-out

- [x] 10.1 `cargo nextest run --all` → 1973 passed, 0 failed. `cargo nextest run --all --features plugins` → 2036 passed, 0 failed (the 63 new tests). Both run with `GIT_CONFIG_GLOBAL=/dev/null`: this machine's global git config sets `commit.gpgsign = true` with `gpg.format = ssh` and no key in the agent, which fails `git::tests::resolve_base_ref_prefers_upstream` (it commits in a tempdir outside the repo, so it does not pick up the repo-local `commit.gpgsign = false`). Pre-existing and unrelated to this change.
- [x] 10.2 Every scenario in the five delta specs maps to a passing test. Two are covered differently than the spec text implies, both recorded in the change's completion notes: the manifest spec's "manifest file cannot be read" is exercised through discovery (`session/` is pure data and does no I/O, so the `Io` variant is constructed by `plugin::discovery`), and the capabilities spec's "a plugin mutates its environment" is asserted as *confinement* — a plugin can shadow a global inside its own VM, which grants no host power and cannot cross a VM boundary.
- [x] 10.3 **Not measurable yet, and recorded rather than approximated.** The plugin host is not invoked from any binary entry point — `grep` over `src/main.rs`, `src/bin/`, `src/app/` and `src/cli/` finds no reference to `plugin::`, `PluginHost`, or `plugin_manifest`. The startup path is therefore byte-identical between a plugins-enabled and a feature-off build, so a `first_frame_ms` comparison would measure only noise. The requirement's real test arrives with the change that starts the host at boot; the structural guarantee it rests on (no VM, no thread when discovery is empty) is covered by `lifecycle::tests::no_plugins_means_no_threads`.
