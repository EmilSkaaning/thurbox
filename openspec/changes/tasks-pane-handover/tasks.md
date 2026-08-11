# Tasks

## 1. The seat

- [x] `src/session/plugin_manifest.rs`: `PaneSlot::Tasks` → `RegionId::Tasks`, with
      the reason ADR-46's rejection is revisited (a position in a column is part of
      the pane).
- [x] `src/app/view.rs`: `render_plugin_panes` paints the new seat from
      `areas.tasks_panel`.
- [x] Tests: the slot round-trips, `seat()` names the region, and the seat is placed
      by a claim alone.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'test(plugin_manifest) or binary(bundled_manifests)'`.

## 2. Seat chrome: the hint row stays the kernel's

- [x] `src/app/mod.rs`: `App::pane_hints(KeyContext) -> Option<&'static [(&str,
      &str)]>` — data, not a closure, so what a seat may draw stays enumerable.
- [x] `src/app/view.rs`: `paint_plugin_pane` reserves the chrome row inside the frame
      and paints it, then lays the tree out in what remains — the same subtraction
      `render_tasks_panel` did.
- [x] Acceptance: the hint row is drawn while the pane holds focus and not otherwise,
      and a row click still selects the row under the pointer.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`.

## 3. The bundled pane takes the pane's identity

- [x] `src/plugin/bundled/tasks/plugin.toml`: `title = "Tasks"`, `slot = "tasks"`,
      `toggle_action = "FocusTasks"`, `feature = "tasks"`, `key_context = "Tasks"`;
      `default_visible = false` stays with its reason.
- [x] `src/plugin/bundled/tasks/init.luau`: rewrite the header — it describes a
      reproduction of a module that no longer exists.
- [x] `tests/bundled_manifests.rs`: add `("tasks", "tasks")` to
      `PANES_DRAWN_IN_A_NATIVE_PANES_PLACE`, so the seed rule and the
      keyboard-declaration rule both apply to it.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_manifests)'`.

## 4. Relocate what the pane declared and `App` owns

- [x] `src/app/task_state.rs`: declare `TaskPaneEntry` here, with a doc saying why it
      is not in `session` and not in `ui`.
- [x] `src/app/mod.rs`, `src/app/search.rs`: use the new path.
- [x] Verify: `cargo check --all`.

## 5. Stop drawing it, and delete the kernel's occupant of the seat

- [x] `src/app/view.rs`: delete `App::render_tasks_panel` and its call; drop
      `tasks_panel` from the `use crate::ui::{…}` list.
- [x] `src/app/mod.rs`: delete `show_tasks_panel`, its initialiser and its two
      `false` writes; `layout_for`'s flag becomes `self.seat_taken(PaneSlot::Tasks)`.
      Keep both focus rescues (feature off, and below 120 columns) — the flag was
      answering "is the pane on screen" for focus, and that question survives it.
- [x] `src/app/key_handlers.rs`: `act_toggle_tasks` flips the pane and focuses it;
      with no pane claiming the keyboard it reports which plugin provides it.
- [x] `src/app/search.rs`: the task-result jump reveals the pane through the same
      door, and reports when there is none; `SearchSnapshot` loses the flag.
- [x] `src/ui/tasks_panel.rs`: **deleted**; `src/ui/mod.rs` drops the module.
- [x] `src/ui/project_list.rs`: the doc reference to `tasks_panel::TaskRow`.
- [x] Verify: `cargo check --all && cargo check --all --no-default-features`.

## 6. The oracle keeps its recordings and loses the builder

- [x] `tests/bundled_tasks_panel.rs`: drop the two `tasks_tree` edges and the
      native-rows plumbing; `Case` builds the published section directly; the
      recordings are the expectation. Rewrite the module note.
- [x] Verify the recordings did **not** move: `git status tests/snapshots/` is empty
      after the deletion.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'binary(bundled_tasks_panel)'`.

## 7. The gates

- [x] `tests/teardown_gate.rs`: `tasks-plugin` becomes `ready`; the handed-over list
      gains it; `EXAMPLE_BLOCKED_PANE` moves to a still-native pane.
- [x] `tests/tasks_pane_input_gap.rs`: **deleted**, with its rows preserved in ADR-53.
- [x] `src/app/acceptance.rs`: the monkey invariant for `TaskList`/`TaskEditor`
      becomes "a pane provides the task list", which is unsatisfiable without the
      plugin host — so that build can never reach the focus.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` and
      `--test architecture_rules`.

## 8. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-53, carrying the retired gate's table.
- [x] `docs/PHASE4-PANE-READINESS.md` §28.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: the second `ready` row.
- [x] `CLAUDE.md`: the tasks-pane section and the keybinding table.
- [x] Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `rumdl check .`, `./scripts/dev/lint-luau.sh`.

## 9. Full verification, and driving it

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `openspec validate tasks-pane-handover --strict`
- [x] Hand-drive: F5 shows and focuses the pane in the native column, `j`/`k` move,
      the central preview follows, `Space` cycles, `n`/`e` open the editor, `r` opens
      the picker, `Esc` leaves, the hint row is there while focused, `[features] tasks
      = false` removes it and F5 says so, and the `--no-default-features` binary
      reports the absence in its own words.
