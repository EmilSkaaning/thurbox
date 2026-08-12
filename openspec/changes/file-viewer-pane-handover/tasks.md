# Tasks

## 1. The seat (`no-file-viewer-seat`)

- [x] `src/session/plugin_manifest.rs`: `PaneSlot::FileViewer` → `RegionId::FileViewer`,
      in `as_str`/`all`/`seat`, with the reason (a position in a column is part of the
      pane — ADR-53's argument, second application).
- [x] `src/app/view.rs`: `render_plugin_panes` paints the new seat from
      `areas.file_viewer`.
- [x] Verify: `cargo nextest run --all -E 'test(plugin_manifest) or binary(bundled_manifests)'`.

## 2. The seat's preemptor (`the-column-has-a-second-kernel-occupant`)

- [x] `src/app/mod.rs`: `App::seat_preempted(PaneSlot) -> bool`, the single rule — the
      file-viewer seat is preempted while `active_review().is_some()`.
- [x] `src/app/view.rs`: `render_plugin_panes` skips a preempted seat;
      `render_file_viewer` becomes `render_review_files` — the review's changed-files
      list is all that is left of it.
- [x] `src/app/mod.rs`: `layout_for` carves the column for the claim **or** an open
      review.
- [x] Acceptance: opening a review over a visible file-viewer pane draws the
      changed-files list and not the pane; closing it restores the pane with no
      keystroke and with its stored visibility intact.
- [x] Verify: `cargo nextest run --all -E 'binary(thurbox)'`.

## 3. Seat chrome becomes a band (`no-frame-node`)

- [x] `src/ui/search_bar.rs` (**new**): `SearchBar` (the bar's kernel state as data) plus
      `render_search_bar` and the four helpers moved from `ui::file_viewer`, with their
      tests.
- [x] `src/app/mod.rs`: `App::pane_hints` → `App::pane_chrome(&self, KeyContext) ->
      Option<PaneChrome>`; `PaneChrome::{Hints, SearchBar}` with each shape's condition
      documented.
- [x] `src/app/view.rs`: `paint_plugin_pane` subtracts a band *before* the frame and a
      hint row *inside* it; `plugin_pane_hints` → `plugin_pane_chrome` (hints follow
      focus, the bar follows its own sub-mode).
- [x] Acceptance: the bar appears at the seat's bottom three rows while a search runs or
      a query is committed, in both focus states, and the tree's box shrinks by exactly
      the band.
- [x] Verify: `cargo nextest run --all`.

## 4. Relocate the model and the window (`the-module-is-the-model-and-the-window`)

- [x] `src/app/file_viewer.rs` (**new**): `FileNode`, `Activation`, `FileViewerState`,
      `enumerate_paths` and their private helpers, moved unchanged, with a doc saying why
      not `session` (it reads directories) and why not `ui` (that module is going).
- [x] `FileRow` **deleted**: `FileViewerState::rows` yields
      `session::pane_context::FileNodeSnapshot`; `App::build_files_snapshot` keeps the
      cap and the cursor rule.
- [x] `src/ui/mod.rs`: `visible_window` moves here; update `ui::plugin_pane` (×2),
      `ui::automation_detail`, `ui::mod`, and the doc link in `ui::theme_picker_modal`.
- [x] `src/app/mod.rs`, `src/app/key_handlers.rs`, `src/app/search.rs`: new paths.
- [x] Verify: `cargo check --all && cargo check --all --no-default-features`.

## 5. Stop drawing it, and delete the kernel's occupant

- [x] `src/app/view.rs`: `render_file_viewer` keeps only the review branch; drop
      `file_viewer` from the `use crate::ui::{…}` list.
- [x] `src/app/mod.rs`: delete `show_file_viewer`, its initialiser and its readers;
      `layout_for`, the `[features]` teardown and the resize keep the **focus rescues**
      it was doing.
- [x] `src/app/mod.rs`: `ScrollTarget::FileViewer` deleted with its recorder (D5).
- [x] `src/app/key_handlers.rs`: `act_toggle_file_viewer` becomes the focus-following
      half plus the "nothing provides this pane" report; the ring stop is the claim.
- [x] `src/app/search.rs`: the search snapshot carries the pane's **stored** visibility;
      a file result reveals the pane through `set_pane_keyboard_visible`.
- [x] `src/app/mod.rs`: `tick_core` syncs the tree to the active session before
      `publish_pane_context`, gated on the pane being on screen.
- [x] `src/app/view.rs`: the footer's `file_viewer_open` reads the claim.
- [x] **Delete `src/ui/file_viewer.rs`**; drop it from `src/ui/mod.rs`.
- [x] Verify: `cargo nextest run --all && cargo nextest run --all --no-default-features`.

## 6. The bundled pane takes the pane's identity

- [x] `src/plugin/bundled/file-viewer/plugin.toml`: `title = "Files"`, `slot =
      "file-viewer"`, `toggle_action = "ToggleFileViewer"`, `feature = "file_viewer"`,
      `key_context = "FileViewer"`, `default_visible = false` with its reason.
- [x] `src/plugin/bundled/file-viewer/init.luau`: rewrite the header — it describes a
      reproduction of a module that no longer exists, and the search bar is now chrome
      rather than "out of scope".
- [x] `tests/bundled_manifests.rs`: add `HandedOver::hidden("file-viewer", "files")`.
- [x] Verify: `./scripts/dev/lint-luau.sh && cargo nextest run --all -E 'binary(bundled_manifests)'`.

## 7. The oracle keeps its recordings and loses the deleted edge

- [x] `tests/bundled_file_viewer.rs`: drop the `file_tree` side; `Case` builds
      `FileNodeSnapshot`s; the ten `.snap` files become the expectation.
- [x] `git status tests/snapshots/` MUST be empty — the recordings are not regenerated.
- [x] Verify: `cargo nextest run --all -E 'binary(bundled_file_viewer)'`.

## 8. The gates

- [x] `tests/teardown_gate.rs`: `file-viewer-plugin` becomes a `ready` row (the
      `pane_is_handed_over` conjunct list, as the three before it);
      `EXAMPLE_BLOCKED_PANE` moves to `session-list-plugin`.
- [x] `tests/code_review_pane_handover_gap.rs`: re-verdict
      `no-second-seat-for-the-changed-files-list` — the seat exists and this list is its
      preemptor — and update `the_review_is_two_seats_not_one`.
- [x] `tests/session_list_pane_handover_gap.rs`: the `visible_window` reference follows
      it to `ui`.
- [x] `tests/file_viewer_pane_input_gap.rs`: **deleted**, its rows preserved in ADR-58.
- [x] Verify: `cargo test --test teardown_gate && cargo test --test architecture_rules`.

## 9. Docs

- [x] `docs/ARCHITECTURE.md`: ADR-58 — the seat, the preemptor, the chrome band, the
      relocation, the lost drag, and the three structural rows that were never granted.
- [x] `docs/PHASE4-PANE-READINESS.md`, `docs/PHASE6-TEARDOWN-READINESS.md`, `CLAUDE.md`.
- [x] Verify: `rumdl check .`.

## 10. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `cargo nextest run --all` and `cargo nextest run --all --no-default-features`
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`, `rumdl check .`
- [x] Hand-driven in `scripts/dev/sandbox.sh --fresh`: the column, the keys, the search
      bar, the review preemption, and the `--no-default-features` build's report.
