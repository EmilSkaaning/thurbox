# Let a plugin declare motion the kernel drives

## Why

A plugin pane today is a still image. The only way to animate it is to push a
new tree per frame, which is exactly what
[ADR-V18](../../../../thurbox-v2/docs/v2/ARCHITECTURE.md) rejects: a push costs a
call into the plugin, a tree rebuild, a conversion and a diff, of which only
the paint is inherent. Worse, it costs those things *whether or not anyone is
looking* — a plugin cannot tell that its pane is hidden, so the idle case pays
too. One installed plugin with a spinner would return thurbox to painting every
tick and undo the demand-driven render loop the whole v1 perf design rests on.

The kernel already animates on its own clock (`SPINNER_FRAMES`, advanced from
the tick counter and gated on something actually working). ADR-V18 is the
generalization of that one working example: **the plugin owns the data, the
kernel owns the clock.**

## What Changes

- **A node may declare `motion`.** The plugin pushes once; the kernel evaluates
  which frame to draw each time it paints, from its own clock. There is no API
  by which a plugin can cause a frame.
- **Motion has identity.** State is keyed by `(pane, node key, signature)` and
  carries an epoch. Re-pushing an identical motion on the same node id
  **continues** the animation instead of restarting it — without this rule any
  plugin that re-pushes on unrelated state changes would pin its spinner to
  frame 0 forever.
- **Motion holds a lease, and only a lease.** A pane whose visible tree contains
  live motion is exempt from the 250 ms redraw floor up to its declared rate.
  The lease drops when the pane hides, when the next tree has no motion, when a
  non-repeating cycle finishes, and when the pane disappears — and the epoch
  drops with it, so a plugin cannot accumulate motion state it no longer shows.
- **Rate is capped twice**: 30 fps per pane and 30 fps aggregate across panes,
  degraded by freezing the greediest leases rather than by stuttering everyone.
- **Reduced motion is honoured app-wide.** `[motion] reduce_motion` renders
  every motion at frame 0 and grants no leases — and freezes thurbox's own
  session-list spinner too, because a user who needs reduced motion needs it
  from thurbox, not from each plugin.
- **The cost is counted.** Four perf counters make "why is thurbox waking up"
  answerable with the same wall-clock-free counters the existing `perf_*`
  acceptance tests assert on.

## Capabilities

### New Capabilities

- `plugin-host/motion`: how a plugin declares something that changes over time,
  what identity and lease guarantees the kernel gives it, how rate is bounded,
  and how reduced motion suppresses it.

## Non-goals

- **Only `cycle` ships.** `marquee`, `pulse`, `blink` and `tween` are named in
  FEATURES-Animation.md §2 and are deliberately not implemented here. `cycle`
  is the general case (a plugin can express a pulse as two frames), and the
  three that are *not* expressible as frames — marquee, and `tween` over layout
  props — need measurement and a layout solver this kernel does not have yet: a
  `marquee` must know its resolved rect, and `tween` must drive `size`/`flex`/
  `padding`, none of which the view tree carries. An unknown kind is rejected
  naming the kinds that exist, so growing the catalogue is additive.
- **No `pauseWhenUnfocused`.** FEATURES-Animation.md §4 lists it as a lease
  drop, and it is deliberately not implemented. A plugin pane stays *visible*
  when focus moves elsewhere, so pausing on focus loss would freeze an animation
  the user can still see — which reads as a hung pane rather than as a saving.
  thurbox's own working spinner keeps animating regardless of which pane holds
  focus for exactly that reason: the animation reports that an agent is busy, not
  that the user is looking. The cost case the knob was for is a pane nobody is
  looking at, and a hidden pane already drops both its lease and its state.
- **No `pty` / `surface` nodes.** ADR-V19, the other half of Phase 3, is not
  started. It is a session-layer change (process supervision, input sinking,
  kitty `REPORT_EVENT_TYPES` on focus) that shares no code with motion.
- **No `plugin doctor` lease listing.** Live leases exist only inside a running
  TUI; `doctor` runs headless and does not execute plugin code, so it has
  nothing to read. The perf HUD and the perf snapshot are where leases surface.

## Impact

`session/motion.rs` (new: the declaration, its bounds, phase evaluation, and
the rate allocator — pure data), `session/view_tree.rs` (a motion node),
`session/settings.rs` + `agent/settings_config.rs` (`[motion] reduce_motion`),
`plugin/view.rs` (conversion and validation), `app/motion_state.rs` (new: the
epoch table and lease bookkeeping), `app/mod.rs` (the tick hook and the
counters), `app/view.rs` and `ui/plugin_pane.rs` (drawing the current frame),
`app/modals.rs` + `ui/settings_modal.rs` (the settings row).

No schema change: motion state is per-process and dies with the TUI, which is
the point — an epoch that outlived a process would be a phase no user ever saw.
