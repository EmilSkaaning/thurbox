# Tasks — declared motion and animation leases

## 1. The declaration (pure data)

- [x] 1.1 `src/session/motion.rs`: `Motion`, `MotionKind::Cycle`, the bounds
      (`MIN_FPS`/`MAX_FPS`/`MIN_FRAMES`/`MAX_FRAMES`/`AGGREGATE_FPS`/`FREEZE_FLOOR_FPS`)
      and `DEFAULT_FPS`.
- [x] 1.2 `Motion::signature`, `Motion::frame_at(elapsed, served_fps)`,
      `Motion::is_live_at`.
- [x] 1.3 `FrameTable`: node key → frame index, `frame()` defaulting to 0.
- [x] 1.4 `allocate_rates`: focused first, ascending declared, freeze below the
      floor.
- [x] 1.5 Tests: phase maths incl. repeat/no-repeat, signature sensitivity,
      allocator within budget / over budget / freeze.

**Verify:** `cargo nextest run --features plugins -E 'test(motion)'`

## 2. The node

- [x] 2.1 `ViewNode::Motion { key, keyed_by_id, motion }` in
      `session/view_tree.rs`; frames are its children so the tree bounds apply.
- [x] 2.2 Tests: node_count and depth include frames.

**Verify:** `cargo nextest run --features plugins -E 'test(view_tree)'`

## 3. Conversion

- [x] 3.1 `plugin/view.rs`: thread the structural path through `convert`.
- [x] 3.2 Parse `motion = { kind, fps, frames, repeat }` on any node; derive the
      key from `id` or the path.
- [x] 3.3 Reject: unknown kind (naming the known ones), frame count out of
      range, malformed field types.
- [x] 3.4 Tests: valid cycle converts; unknown kind names the catalogue; too
      few / too many frames rejected; fps clamped both ends; id keys the node;
      id-less falls back to the path; frames count against the node budget.

**Verify:** `cargo nextest run --features plugins -E 'test(plugin::view)'`

## 4. Epoch table and leases

- [x] 4.1 `src/app/motion_state.rs`: `MotionState` with the epoch map, per-pane
      frame tables, and counters.
- [x] 4.2 `sync(panes, focused, reduce_motion, now) -> bool` — collect, GC,
      allocate, resolve, report change.
- [x] 4.3 Tests: identical re-push keeps phase; signature change restarts; hide
      drops the lease and the state; a finished non-repeating cycle drops the
      lease; state does not grow across repeated pushes; reduced motion renders
      frame 0 and denies.

**Verify:** `cargo nextest run --features plugins -E 'test(motion_state)'`

## 5. App and renderer wiring

- [x] 5.1 `App::motion` field; call `sync` from `tick_core`; mark dirty only on
      a changed frame table.
- [x] 5.2 `PerfCounters`: `motion_leases`, `motion_frames`, `motion_denied`,
      `motion_frozen` (+ `delta`).
- [x] 5.3 `app/view.rs` passes the pane's `FrameTable` to the renderer.
- [x] 5.4 `ui/plugin_pane.rs` draws the resolved frame; height is the max over
      frames so an animation cannot make the layout jitter.

**Verify:** `cargo nextest run --features plugins`

## 6. Reduced motion

- [x] 6.1 `[motion] reduce_motion` in `session/settings.rs` (+ `Settings`
      default, `restart_only_differs` unaffected — it applies live).
- [x] 6.2 Loader + seed comment in `agent/settings_config.rs`.
- [x] 6.3 Mirror onto `App` via `apply_live_settings`; freeze
      `advance_spinner_frame`.
- [x] 6.4 Settings-panel row (`SettingsField::MotionReduce`, its `[motion]`
      section header).
- [x] 6.5 Tests: default off; on freezes the spinner and denies plugin motion;
      the panel round-trips it through `settings.toml`.

**Verify:** `cargo nextest run --all`

## 7. Acceptance (the exit criterion)

- [x] 7.1 A visible 8 fps animated pane advances `motion_frames` at ~8/simulated
      second, not once per tick.
- [x] 7.2 A hidden animated pane leaves `motion_frames` at zero and the idle
      paint rate identical to no plugin at all.
- [x] 7.3 An identical re-push after time has passed renders the later frame.
- [x] 7.4 A real frame draws the animated content (end to end through
      `App::view`).

**Verify:** `cargo nextest run --features plugins -E 'test(motion)'`

## 8. Docs

- [x] 8.1 `CLAUDE.md`: motion under the plugin section.
- [x] 8.2 `docs/CONFIG.md`: `[motion] reduce_motion`.
- [x] 8.3 `docs/PERFORMANCE.md`: the four counters and the lease rule.

**Verify:** `rumdl check .`
