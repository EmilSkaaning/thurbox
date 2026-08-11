# A pane renders when a source it reads moves, not once a second

## Why

The plugin render worker renders every visible pane and then waits out a **fixed
1 s interval** in ten 100 ms slices, serving key requests. Nothing tells it that
kernel state moved. So the two halves of the same fact move at very different
speeds:

| | latency |
|---|---|
| the native pane's cursor moves | next frame — single-digit ms |
| a plugin pane's copy of that cursor moves | the worker's next cycle — **up to 1 s** |

The session-list spike fixed a bar of 5 ms of added latency on a selection change
and made its verdict conditional on the render being event-driven. It is not, so
the bar is missed by roughly 200×. The spike's other open bar is idle CPU: the
worker re-renders whether or not anything moved, which is 0.87 ms/s of Luau at 20
sessions and 14.5 ms/s at 200, against v1's zero for the same pane.

`docs/PHASE4-PANE-READINESS.md` §13 argued the staleness was tolerable *because* a
plugin pane is a hidden reproduction, so the surface the user watches is still the
kernel's. §21 and §22 closed the seat and the toggle, and a **handover inverts that
argument entirely**: the stale pane becomes the only pane. It is the last open row
of §14's five handover requirements, and the worst case is the info panel — live
CPU and memory gauges plus per-automation countdowns, all updating once a second.

Two closures were considered and rejected when the gap was filed, and both
objections rest on an assumption that is measurable:

- *nudge the worker whenever the published snapshot changes* — rejected because
  "the snapshot carries host CPU and memory, so it changes on nearly every tick: a
  1 Hz poll becomes a ~100 Hz one". The snapshot is already change-gated, and the
  values in it move at their **collection** cadence, not the tick's:
  `METRICS_REFRESH_TICKS` is 100 ticks (~1 s) and the countdowns are whole seconds.
  So the publish rate is ~1–2 Hz at rest, not ~100 Hz. But the objection is not
  empty either: agent activity text can change on many consecutive ticks, so an
  unbounded nudge is a real risk and needs a rate policy rather than a dismissal.
- *nudge only when the session section changes* — recorded as "probably right
  eventually". It is what this change does, generalised: a pane renders when a
  source **it** reads moves, so the session list does not re-render because host
  CPU moved.

## What Changes

- **A pane's sources are named.** A new `PaneSource` — `sessions`, `metrics`,
  `automations`, `tasks`, `files`, `review`, plus `plugin-state` — with
  `Capability::source()` mapping each state-reading capability to exactly one, and
  a `SourceSet` bitset. The mapping is exhaustive over `Capability`, so a new
  capability cannot be added without deciding what it reads.
- **A publication says what moved.** `PaneContext::changed_sources` compares two
  snapshots and returns the sources that differ, destructuring both by name with no
  rest pattern — so a field added to the snapshot cannot slip into no source. It is
  also the publisher's change gate, replacing the whole-value comparison, and a test
  pins that the two agree.
- **The publisher nudges the worker** with the sources that moved, over the channel
  that already carries key requests (one channel, one shutdown path). A visibility
  change nudges "every pane", since a pane the worker was skipping must render as
  soon as it is on screen.
- **The worker's trigger is a pure state machine.** `plugin::render_trigger` decides
  what to render and when to look again, given what it has been told and a clock
  passed in — so the policy is unit-testable even though the loop that drives it
  lives in `src/main.rs` and cannot be.
- **A rate ceiling replaces the interval.** A pane renders at most once per
  `PLUGIN_RENDER_MIN_INTERVAL` (100 ms, so ≤10 Hz — the spike's own bar, and tighter
  than the kernel's 250 ms forced-redraw floor). Coalescing is the ceiling's only
  job: a change arriving after a quiet period renders immediately.
- **The timer survives for exactly one case, named.** A pane that reads its plugin's
  own durable state (`state-read`) depends on a source the kernel cannot observe — a
  service half may write it with nothing on the UI thread knowing — so that pane,
  and only that pane, is still re-rendered on the source-file poll's cadence. No
  bundled plugin declares it, so the bundled set's idle render cost becomes zero.
- **The idle paint property is asserted on counters.** Two new perf counters
  (`plugin_renders_applied`, `plugin_renders_changed`) make "a re-render producing
  the same tree costs no repaint" a failing test rather than a claim.

## Non-goals

- **No pane is handed over and nothing is deleted.** All six native renderers are
  still what the interface draws, and every row of `tests/teardown_gate.rs` stays
  blocked.
- **No plugin gains a way to ask for a frame.** The trigger is the kernel's, exactly
  as declared motion is (ADR-V18); a plugin that could request a render could defeat
  the demand-driven loop.
- **No filesystem-notification dependency.** The source-file poll that drives hot
  reload keeps its cadence and its reasons; only the *render* stops riding it.
- **The residual latency is not hidden.** A change arriving within 100 ms of the
  previous render waits out the remainder, so the worst case is 100 ms rather than
  the 5 ms the spike's bar asks for. That is stated in the audit, with the measured
  publish rate that makes it rare.
- **Sub-pane granularity is not attempted.** A pane that reads `sessions` re-renders
  when any session field moves, not only the one its rows show.

## Impact

- Affected specs: `plugin-host/panes` (three ADDED requirements),
  `plugin-host/kernel-state` (one MODIFIED), `migration/phase-4` (one MODIFIED).
- Affected code: `src/session/plugin_manifest.rs`, `src/session/pane_context.rs`,
  `src/plugin/render_trigger.rs` (new), `src/plugin/lifecycle.rs`,
  `src/plugin/mod.rs`, `src/app/mod.rs`, `src/app/metrics_state.rs`,
  `src/app/acceptance.rs`, `src/main.rs`, `tests/architecture_rules.rs`.
- The compile-time gate is the existing `plugins` feature, which is in the default
  set since Stage B. `session::PaneSource` and `changed_sources` are pure data and
  compile in both configurations (the snapshot type already does); the trigger, the
  worker and the nudge are behind `#[cfg(feature = "plugins")]`, so
  `--no-default-features` gains nothing and loses nothing.
- `App::plugin_keys` becomes `App::plugin_worker` and carries an enum, so the
  acceptance harness's channel type changes with it.
