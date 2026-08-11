# Tasks

## 1. Publish the session list

- [x] `src/session/pane_context.rs`: add `SessionRowSnapshot`, `SessionListSnapshot`
      and `MAX_SESSION_ROWS`; add the `session_list` field to `PaneContext`.
      Document what the section does **not** carry (a composed row, a pre-fitted
      text, the animation's frames, a resolved status text).
- [x] `src/app/mod.rs`: `build_session_list_snapshot`, called from
      `build_pane_context`; reuse the cached session order when its signature
      still matches, so publishing costs no regrouping.
- [x] Verify: `cargo nextest run -E 'test(pane_context)'`,
      `cargo nextest run -E 'test(session_list_snapshot)'`.

## 2. Bind it, under the capability that already exists

- [x] `src/plugin/kernel_state.rs`: `session_list_table` + `build_session_row`,
      with unit tests for the wire shape (a row's flags cross even when false, an
      absent field is an absent key, offsets cross as a one-based array).
- [x] `src/plugin/capabilities.rs`: expose `sessionList` under
      `Capability::Sessions`, beside `activeSession`.
- [x] `src/session/plugin_manifest.rs`: **no change** — assert that in the port's
      test rather than editing the vocabulary.
- [x] Verify: `cargo nextest run --features plugins -E 'test(kernel_state)'`.

## 3. Make the native pane draw its rows from the tree

- [x] `src/ui/plugin_pane.rs`: `pub fn line_spans(node, width, palette, frames)`
      — the existing inline walk, exposed so a native pane resolves a `Fill`'s
      residue by the same arithmetic a plugin pane does.
- [x] `src/ui/project_list.rs`: add `SessionRow`, `SessionListItem`,
      `resolve_items` (the geometry step: group labels, the cross-group child rule,
      fitting the agent text by character count and dropping it under four
      columns) and `session_item_node` / `session_list_tree` (the presentation
      step, geometry-free). Working rows declare motion; retain the pre-port span
      builders as `#[cfg(test)]` oracles.
- [x] `src/app/view.rs`: pass `spinner_frame: usize` instead of the resolved
      frame string, so `project_list` can fill a `FrameTable`.
- [x] Verify: `cargo nextest run -E 'test(project_list)'` — including the new
      oracle test that the tree paints what the pre-port span builder painted, at
      several widths and for every row variant.

## 4. The bundled plugin

- [x] `src/plugin/bundled/session-list/plugin.toml` — `capabilities = ["render",
      "sessions"]`, `default_visible = false`, and a comment saying why it is off.
- [x] `src/plugin/bundled/session-list/init.luau` — the pane.
- [x] `src/plugin/discovery.rs`: register it in `BUNDLED`.
- [x] `src/plugin/bundled/thurbox.d.luau`: `SessionRow`, `SessionList`,
      `sessionList`.
- [x] Verify: `./scripts/dev/lint-luau.sh`.

## 5. The measurement

- [x] `tests/bundled_session_list.rs`:
      `the_plugin_builds_the_native_panes_view_tree` over eleven content variants
      (one row per status, several repo groups, nesting, a cross-group child,
      remote and worktree marks, reported text, a running search, multi-byte
      names, no cursor, everything at once, no repo);
      `the_comparison_size_adjusts_nothing`; `the_compared_tree_is_a_whole_pane`;
      `the_plugins_animation_is_the_kernels_animation`;
      `the_plugin_declares_every_power_it_uses`;
      `the_host_surface_needed_no_new_node`; and one test per enumerated
      divergence (`the_empty_pane_is_the_one_place_the_plugin_differs`,
      `the_two_panes_window_a_long_list_by_different_rules`,
      `non_ascii_whitespace_is_trimmed_by_the_kernel_only`).
- [x] `src/ui/project_list.rs` tests: the paint oracle against the retained
      pre-port span builders (`the_tree_paints_what_the_span_builder_painted`,
      `every_spinner_frame_paints_what_the_span_builder_painted`,
      `the_header_tree_paints_what_the_span_builder_painted`), the geometry
      assertions (`the_tree_carries_no_geometry`,
      `the_agent_text_is_fitted_or_dropped`), the motion declaration
      (`a_working_row_declares_its_animation`), and the pinned latitude
      (`the_only_divergence_is_a_blank_cells_foreground`).
- [x] Verify: `cargo nextest run --features plugins -E 'test(bundled_session_list)'`.

## 6. Record it

- [x] `docs/PHASE4-PANE-READINESS.md`: a new section for this port — what was
      ported, what was not, the spike's three conditions re-checked, the render
      interval measured, and the two new open vocabulary rows (a centred line, a
      border overlay).
- [x] `docs/ARCHITECTURE.md`: the ADR for this port.
- [x] `CLAUDE.md`: the bundled set, the new reader, and the spinner as declared
      motion.
- [x] Verify: `rumdl check .`.

## 7. Gate

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `cargo tree --edges normal | grep -c mlua` → 0
- [x] `git status src/app/snapshots` → clean
