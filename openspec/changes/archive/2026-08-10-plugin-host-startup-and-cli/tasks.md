## 1. Host reporting shape

- [x] 1.1 Add a `PluginReport` in `src/plugin/lifecycle.rs` (or a new `report.rs`) carrying the statuses plus the discovery problems, so one value answers both `list` and `doctor` without the CLI reaching into internals.
- [x] 1.2 Add a compile-time assertion that `PluginHost: Send` (design D2), so a future non-`Send` field fails the build rather than forcing a redesign.
- [x] 1.3 Add `PluginHost::start_detached` — discovery + `start_all` on a worker thread, returning a receiver for the finished host and logging every failure with plugin, transition, and cause.

**Verify:** `cargo nextest run -E 'test(plugin::lifecycle)' --features plugins`

## 2. TUI boot and shutdown

- [x] 2.1 In `src/main.rs`, start the host via `start_detached` during boot, before the render loop and without waiting on it. `App` gains no field (design D1).
- [x] 2.2 At shutdown, collect the host with a bounded wait and call `stop_all`; an absent host is "nothing to stop", not an error (design D3).
- [x] 2.3 Confirm the boot path calls no plugin code on the UI thread — the only synchronous work is spawning the worker.

**Verify:** `cargo build --features plugins`, manual boot with a plugin installed

## 3. CLI subcommand

- [x] 3.1 Create `src/cli/plugins.rs` with `list`, `status [<name>]`, and `doctor`; register it in `src/cli/mod.rs` behind `#[cfg(feature = "plugins")]`.
- [x] 3.2 Wire the subcommand into `src/bin/thurbox-cli.rs`'s clap enum behind the same `cfg`, so it is absent from help without the feature (design D5).
- [x] 3.3 `list`/`status` run the full start sequence; `doctor` runs discovery only (design D4).
- [x] 3.4 `status <name>` for an unknown plugin exits with an error naming it, rather than an empty success.
- [x] 3.5 Emit human/JSON output through the existing shared formatting helper the other subcommands use, honoring `--json` / `--pretty` / `--text`.
- [x] 3.6 Extend `cli`'s `allowed_path_only` in `tests/architecture_rules.rs` with `plugin`, and reach it only via fully-qualified paths (design D6).

**Verify:** `cargo test --test architecture_rules --features plugins`

## 4. Tests

- [x] 4.1 Lifecycle tests for the new requirements: both binaries discover the same set, a hanging plugin does not block others, failures are logged, shutdown stops everything.
- [x] 4.2 CLI tests: mixed healthy/failed listing, empty listing, status for a failed plugin naming its transition, status for an unknown name failing, doctor reporting invalid/overridden/conflicting/unreadable, doctor clean.
- [x] 4.3 A test that a plugin hanging in `init` still lets the host report and shut down, exercising the bounded collection.

**Verify:** `cargo nextest run --all --features plugins`

## 5. Docs

- [x] 5.1 Add the `plugin` verbs to `CLAUDE.md`'s thurbox-cli subcommand list and the plugin directory to its config paths.
- [x] 5.2 Add the plugin directory to `docs/CONFIG.md`'s config-location table.

**Verify:** `rumdl check .`

## 6. Close-out

- [x] 6.1 `cargo nextest run --all` → 1973 passed, 0 failed. `--features plugins` → 2052 passed, 0 failed (16 new). Both with `GIT_CONFIG_GLOBAL=/dev/null` for the pre-existing commit-signing issue recorded in the previous change.
- [x] 6.2 Clippy clean with `--features plugins`, with default features, and with `--all-features`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` clean; `rumdl check .` clean; `cargo fmt --all --check` clean.
- [x] 6.3 **Measured.** 5 runs each, throwaway `THURBOX_CONFIG_DIR`/`THURBOX_DATA_DIR`/`TMUX_TMPDIR` per run, debug builds, TUI driven inside a scoped tmux socket. Feature off: 191, 184, 177, 173, 171 ms (median **177**). Feature on, no plugins installed: 175, 174, 179, 171, 201 ms (median **175**). The two are indistinguishable — the difference is smaller than run-to-run spread — so the budget holds with margin. Boot does discovery only; VM startup is off-thread by construction (design D2).
- [x] 6.4 **End-to-end check with a real plugin.** A hand-written `plugin.toml` + `init.luau` in an isolated config dir loads, runs `init`, calls the capability-gated `thurbox.log`, and reports `running` with `capabilities: log`; a sibling plugin with a malformed name is rejected and named by `doctor`. Piped output is valid JSON (warnings go to stderr), and `status <unknown>` exits 1.
