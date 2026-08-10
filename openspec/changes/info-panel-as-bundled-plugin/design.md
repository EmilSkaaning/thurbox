# Design — the info panel as a bundled plugin

## 1. The shape of the state channel

`docs/PHASE4-PANE-READINESS.md` §2 already named the precedent, and it is the
right one: `session::spawn_contribution` is a process-wide
`RwLock<Option<Registry>>` living in pure data, written by whichever binary owns
the data and read by whoever needs it. Applied here:

- `session::pane_context` holds `PaneContext` (pure data) and a
  `static CONTEXT: RwLock<Option<PaneContext>>` with `publish()` / `published()`.
- `app` builds it from `App` state and publishes it on the tick.
- `plugin::kernel_state` converts a `PaneContext` section into a Lua table when a
  binding is called.

**Owners, checked against `tests/architecture_rules.rs`:**

| New type | Module | Why it may live there |
|---|---|---|
| `PaneContext`, `SessionSnapshot`, `SystemSnapshot`, `AutomationSnapshot`, `StatusSnapshot`, `GitSnapshot`, `AgentMetricsSnapshot`, `UsageSnapshot`, `UsageWindowSnapshot` | `session::pane_context` | `session`'s allowlist is empty, and these reference only `std` and `super::SessionStatus`/`super::view_tree::StyleToken` — the same reach `session::view_tree` already has |
| the three Lua table builders | `plugin::kernel_state` | `plugin` may reach `session` + `paths`; this reaches `session` only |
| `App::publish_pane_context`, `App::build_pane_context` | `app` | `app` is the coordinator and already owns every input |

No allowlist edit, no `CLAUDE.md` architecture-section edit, no
`docs/CONSTITUTION.md` edit — because there is no new edge. That is the point of
routing through `session`: the alternative (a binding that holds an `&App`) would
have needed `plugin → app`, which is refused, and would have put plugin code on
the UI thread besides.

### Rejected: pass the state to `render` as an argument

`render(paneId)` could become `render(paneId, state)`. Rejected: it grants every
plugin a session's name, branch and activity text with no capability declared —
exactly the reach `plugin-host/capabilities` exists to make reviewable — and it
would break every existing plugin's signature to do it.

### Rejected: one capability for all kernel state

A single `state` or `kernel` capability is one enum variant instead of three.
Rejected because the capability list is the install prompt: "reads your sessions"
and "reads this machine's CPU and memory" are different questions to ask a user,
and collapsing them means the smallest pane that wants a session name also
demands host telemetry. The cost of three is three enum arms and three `if
granted.has(..)` blocks.

### Residual, named rather than fixed: two reads in one render may straddle a publication

Each reader calls `pane_context::published()` independently, so a plugin that
calls two of them in one `render` can, in principle, get section A from
publication *N* and section B from *N+1*. A publication is atomic — the lock
replaces the whole value, so no reader ever sees half a snapshot — but the three
reads are not one transaction.

Making it exact is possible and was considered: refresh a thread-local snapshot
once per render inside the plugin thread and have the readers use it, which would
also cut three clones to one. It is not done here because it introduces a *second*
read path (a reader called from `init` or a command would have to fall back to
`published()`), and "which snapshot did I get" is a question worth answering with
a measurement rather than inventing on the way past. The observable effect today
is one section being one worker cycle newer than another in a pane that redraws
continuously.

### Rejected: a channel or a callback instead of a lock

A channel would make the reader block on a producer that is on the UI thread.
The whole design constraint is that the plugin worker never waits on the render
loop and the render loop never waits on a plugin. A lock read that clones is the
cheapest thing that satisfies both.

## 2. What the snapshot carries, and the line it draws

The sandbox loads `TABLE | STRING | MATH | BIT | BUFFER | COROUTINE | VECTOR`
(`plugin::runtime`) — **no `os`, no `io`, no path library**. So the line is
forced rather than chosen:

**The kernel resolves what the plugin cannot compute.** A clock
(`resets_in_secs`, `due_in_secs`), a path basename (`repo_name`,
`additional_dir_names`), a cross-record lookup (`parent_name`), and a rendering
decision the kernel owns (a status's label, glyph, and style token).

**The plugin composes every string it displays.** `format_bytes`,
`format_bytes_pair`, `format_tokens`, `format_duration`, `format_cost`,
`format_countdown_secs`, the section headings and every `label: value` row are
reimplemented in Luau against raw numbers.

The second half is the load-bearing one. Publishing `"8.0/16.0 GB"` would have
made the port trivial and worthless: the plugin would be arranging strings the
kernel formatted, and nothing would have been learned about what a third-party
pane can do. Publishing `memory_used: 8589934592` means the plugin really does
own its presentation — and it is what surfaced the finding in §6.

The status triple deserves its own note. `StyleToken::for_status` exists
precisely "so two panes cannot disagree about which colour a state gets"
(`session::view_tree`); a plugin re-deriving `"status_" .. name` would be a
second such mapping, in a language where nothing checks it. So the snapshot
carries the token the kernel resolved. Same argument for the glyph: `◆` is not a
plugin's decision.

## 3. Where publishing happens, and what stops it costing anything

`App::publish_pane_context` runs at the end of `tick_core` — the deterministic
half of the tick, so `Harness::tick` exercises it and the acceptance tests can
assert on it with no runtime.

Two gates, in order:

1. **Demand.** `pane_context::readers_present()` is a relaxed `AtomicBool` load.
   `PluginHost::publish_state_demand` sets it from the grants of what is
   *running*, at every entry point that can change that — `start_all`, `reload`,
   `reset`, `stop_all` — so a plugin that failed to start or was stopped stops
   being counted. In a build without the plugin feature nothing ever sets it, so
   the publisher is one atomic load per tick and returns.
2. **Change.** The built `PaneContext` is compared against the last published one
   (held on `App`) and the lock is written only on a difference.

Counters `pane_context_builds` / `pane_context_publishes` on `MetricsState::perf`
make both assertable without wall-clock timing, in the style
`docs/PERFORMANCE.md` established.

### Rejected: an input signature instead of building-then-comparing

`App::session_order_signature` avoids rebuilding by hashing the inputs first.
Rejected here: a signature over *these* inputs has to touch every field the
snapshot touches, so it saves allocations, not traversal — and it introduces a
second description of "what the snapshot depends on" that can drift from the
snapshot itself. That drift is a silent-staleness bug, which is worse than ~30
short allocations on a tick that already only runs when a plugin is installed.
If a future pane makes the build genuinely hot, the signature is still available.

One value *is* quantised on the way in, for exactly this gate's sake: an
automation's countdown is published in whole **seconds**, the granularity it is
displayed at. Milliseconds would carry no extra information and would differ on
every tick, so a single pending automation would write the slot a hundred times a
second and the change gate would do nothing.

**Known consequence, accepted:** if a published metric were `NaN`, `PartialEq`
would report a difference every tick and the lock would be written every tick.
That is bounded work with no correctness effect, and no producer in the tree
yields `NaN` for a percentage.

### Rejected: publishing from the render path

Publishing when the pane is painted would make the snapshot exactly as fresh as
the frame. Rejected: `App::view` takes `&self`, the plugin worker reads
asynchronously, and tying kernel state to paint order would mean a pane that is
off screen sees stale state — a bug that only appears for the second pane.

## 4. Making `info_tree` a pure function of its inputs

`info_tree` called `epoch_now_secs()` to build the usage countdown, so its output
depended on the wall clock. `now: u64` becomes a parameter, resolved by
`render_info_panel`. Two reasons:

- The differential test compares the plugin's tree to `info_tree`'s. With an
  internal clock the two are built microseconds apart and disagree whenever a
  minute boundary falls between them.
- It completes the property Phase 0 started. `the_tree_carries_no_geometry`
  asserts the tree does not depend on width; the tree should not depend on the
  clock either, for the same reason — a plugin has neither.

The pinned frame is the check that this changed no output.

## 5. The differential test, and where it lives

The centrepiece: build a `SessionInfo` (+ metrics, usage, automations), publish
the `PaneContext` derived from the same values, run the bundled plugin's `render`
through a real `PluginHost`, and assert the resulting `ViewNode` **equals**
`info_tree(...)`. `ViewNode` derives `PartialEq`, and the same renderer paints
both, so tree equality is byte-identity of the pane without comparing frames.

It lives in `tests/bundled_info_panel.rs`, because it is the one place that must
see both `ui::info_panel::info_tree` and `plugin::PluginHost`. Under `src/` only
`app` may reach both, and putting it in `src/app/acceptance.rs` would have worked
— but an integration test is not part of the library's module graph at all, so
the allowlist stays untouched and the file needs no `plugins`-gated island inside
an already-large module. It runs the plugin from `src/plugin/bundled/info-panel/`
directly via `discovery::discover_in`, so the test checks the shipped source
rather than a materialized copy.

### Rejected: comparing rendered frames instead of trees

Slower, and it would pass for the wrong reason — two different trees can paint
the same at one width. Tree equality is strictly stronger and localises a failure
to a node.

## 6. What had to be widened, and what did not

**Widened:** the state channel and its three capabilities (PHASE4 §2). That is
the whole of it.

**Not widened: the view tree.** The plugin needs `list`, `paragraph`, `divider`,
`gauge` and `text` with eight distinct tokens, and every one already exists. So
Phase 0's catalogue work is confirmed by an independent consumer rather than only
by the pane that motivated it.

**Not closed, and measured rather than asserted:**

- **Freshness.** The render worker polls on a ~1 s cycle
  (`PLUGIN_RENDER_SLICE × PLUGIN_RENDER_SLICES` in `main.rs`), so the plugin's
  CPU gauge lags the native pane's by up to a second.
  `docs/SPIKE-SESSION-LIST.md` already fixed *event-driven render* as a condition
  of the session-list port; this port is the second pane to want it, which is
  worth recording and is not this change's to fix.
- **Formatter duplication.** Every plugin that displays a byte count will
  reimplement `format_bytes`. A `thurbox.format.*` helper table would fix it and
  is deliberately not added here: it should be designed from two or three panes'
  needs, not from one, and adding it now would also destroy this port's evidence
  that a plugin can own its own presentation.

## 7. Tightening the teardown gate

`tests/teardown_gate.rs`'s `pane()` probe answers "is there a bundled plugin
directory named after this pane?". After this change that answer becomes `true`
for the info panel while `src/app/view.rs` still calls
`info_panel::render_info_panel` — and the gate would then permit deleting
`src/ui/info_panel.rs`, which is the pane every user sees.

So the probe becomes a conjunction: the plugin exists **and** `src/app/view.rs`
no longer names the native renderer module. Every one of the seven native pane
renderers is referenced from that file today, so the rule is uniform, and the
verdict now means *handover* rather than *coexistence*. The row stays
`ready: false`, `readiness_is_derived_from_the_verdicts` keeps passing unchanged,
and a new test asserts the interesting half — the plugin exists and the row is
still blocked — so nobody "fixes" the probe back to existence.

### Rejected: leaving the probe alone and not shipping the directory under the audited name

Naming the plugin something the probe does not match would have kept the gate
green by hiding from it. That is the opposite of what the gate is for.

## 8. Visibility and the default interface

The manifest declares `default_visible = false`. `PaneDecl::default_visible`
defaults to `true`, which is right for a plugin a user installed on purpose and
wrong for a second bundled pane nobody asked for: shipping it visible would
change every user's layout and put two info panels on screen. `F10` still
toggles `plugin_panes.first_mut()` — `hello`, since panes are ordered by plugin
name — so the info-panel pane is currently reachable only by the stored
visibility state. That is PHASE4 §5, already open and named there; it is not
worked around here.
