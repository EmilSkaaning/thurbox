# Tasks

## 1. The seam

- [x] `src/session/plugin_mutations.rs` (new): the `KernelWriter` trait (five
      methods, ids in and `bool` out, `Result<_, String>`) plus
      `KernelWriterFactory`; registered in `src/session/mod.rs`.
- [x] `src/storage/automations.rs`: `set_automation_enabled_rescheduled` — enable
      recomputes `next_run_at` from the schedule, disable clears it — with a unit
      test for both directions.
- [x] `src/app/automation.rs`: `toggle_automation_by_id` calls it, so the rule has
      one home.
- [x] `src/storage/plugins.rs`: `DbKernelWriter`, its own connection like
      `DbPluginStore`; a task status string is parsed against `TaskStatus` and an
      unknown one is an error naming the accepted values.
- [x] Verify: `cargo nextest run -E 'test(automation_enabled) + test(kernel_writer)'`.

## 2. The capabilities and their bindings

- [x] `src/session/plugin_manifest.rs`: `Capability::{TasksWrite, AutomationsWrite}`
      (`tasks-write` / `automations-write`), in `all()`, `as_str`, and **not** in
      `reads_kernel_state` (a write demands no snapshot).
- [x] `src/plugin/capabilities.rs`: `build_module_table` takes the writer;
      `setTaskStatus` + `deleteTask` under `tasks-write`, `setAutomationEnabled` +
      `runAutomation` + `deleteAutomation` under `automations-write`; tests that
      each grant inserts only its own bindings, that neither implies the matching
      reader, and that no binding named `createTask`/`updateAutomation`/`exec`/`sql`
      exists under either.
- [x] Verify: `cargo nextest run --features plugins -E 'test(capabilities)'`.

## 3. Threading the factory

- [x] `src/plugin/runtime.rs`: `spawn_half` takes the writer factory and builds it
      on the plugin's thread beside the store.
- [x] `src/plugin/lifecycle.rs`: `PluginHost` holds an optional factory
      (`with_kernel_writer`), passes it to every plugin it starts, and
      `start_detached` accepts one; the `Send` assertion still holds.
- [x] `src/plugin/service.rs`: the service half takes it too.
- [x] `src/main.rs`, `src/cli/{plugins,commands,automations}.rs`: build the factory
      where the store factory is already built.
- [x] Verify: `cargo nextest run --features plugins -E 'test(lifecycle) + test(service)'`.

## 4. End to end from a plugin's VM

- [x] `src/plugin/lifecycle.rs` tests: a plugin granted `tasks-write` changes a
      real task's status and soft-deletes one through its own binding, against a
      temp database; a plugin without the capability finds no binding.
- [x] `src/plugin/bundled/thurbox.d.luau`: the five signatures, each documented
      with what it does *not* do.
- [x] Verify: `cargo nextest run --features plugins -E 'test(mutat)'`,
      `./scripts/dev/lint-luau.sh`.

## 5. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-35 — the closed operation list, why the kernel
      still fires automations, the residual reach, the seam.
- [x] `CLAUDE.md`: the two capabilities in the plugin-host paragraph.
- [x] `docs/CONFIG.md`: the capability names in the plugin manifest reference (if
      it lists them).
- [x] Verify: `rumdl check .`.

## 6. The row id a write addresses

- [x] `src/session/pane_context.rs`: `TaskSnapshot::id` / `AutomationSnapshot::id`
      — found by driving the pane by hand, where `entry.id` was `nil` and the
      binding had nothing to address. `src/ui/tasks_panel.rs` carries it on
      `TaskPaneEntry` so the snapshot is built from the list the pane resolved.
- [x] `src/plugin/kernel_state.rs`: `id` on both wire tables, with the assertions.
- [x] `tests/global_search_pane_gap.rs`: the read-only row's probe now
      distinguishes a **view** write from a **record** write — a record write does
      not close it — plus `a_record_write_is_not_the_write_the_strip_needs`.
- [x] `docs/PHASE4-PANE-READINESS.md` §10: the row and the correction.
- [x] Verify: `cargo nextest run --all --features plugins`,
      `cargo nextest run --test global_search_pane_gap`.

## 7. Whole-tree verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate --features plugins`
- [x] By hand: a throwaway plugin in `scripts/dev/sandbox.sh --plugins` that
      deletes a task on a keypress, with `thurbox-cli task list` before and after.
