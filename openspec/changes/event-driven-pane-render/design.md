# Design

## The shape

Four pieces, three of them pure:

| Piece | Module | Kind |
|---|---|---|
| `PaneSource` / `SourceSet`, `Capability::source()` | `session::plugin_manifest` | pure data |
| `PaneContext::changed_sources` | `session::pane_context` | pure function |
| `RenderTrigger` — what to render, and when to look again | `plugin::render_trigger` | pure state machine, clock passed in |
| the loop that drives it | `src/main.rs` | glue |

The split exists because of where the loop lives. `spawn_plugin_render_loop` is in
`src/main.rs`, which is a binary: nothing under `tests/` or `src/` can call it, and a
policy written inline there is a policy no test can reach. Everything that *decides*
therefore moves out, and what stays in `main.rs` is the part a test would not assert
anyway — a channel receive, a filesystem stat, a `Vec` sent down a pipe.

## Where the vocabulary lives, and why not with the snapshot

`PaneSource` goes next to `Capability`, not next to `PaneContext`.

The question a source answers is "what can this pane read", which is a property of
the **grant**, and the seventh member settles it: `plugin-state` is a source a pane
reads but is not part of the published snapshot at all. A type in
`session::pane_context` with a member that has nothing to do with the snapshot would
be misfiled. `PaneContext::changed_sources` then imports the vocabulary rather than
defining it, and documents (and tests) that it never returns `plugin-state`.

Both modules are inside `session`, so `tests/architecture_rules.rs` needs no change:
`session`'s allowlist is empty because it may reference no *other* top-level module,
and this is intra-module.

`plugin::render_trigger` is a new module under `src/plugin/`, whose allowlist is
`["session", "paths"]`. It reaches `session::plugin_manifest` for `SourceSet` and
nothing else — no `ui`, no `app`, no `agent`. The allowlist entry for `plugin` is
unchanged; a new *file* inside an already-declared module needs no new row.

## The rate policy, and the number

`PLUGIN_RENDER_MIN_INTERVAL = 100 ms`, i.e. ≤10 render passes per second.

Three reasons for that number rather than a smaller one:

- it is the session-list spike's own bar 1 ceiling (`view.push` rate ≤ 10 Hz
  sustained), so the trigger cannot break the bar it was written against;
- it is tighter than the kernel's `FORCE_REDRAW_INTERVAL` of 250 ms, so a plugin pane
  can never be more than one forced frame behind the interface around it; and
- it is the ceiling the gap's own filing named ("the spike's own 10 Hz ceiling").

The policy is **coalescing, not delaying**: `due(now)` answers `Now` whenever the
interval has elapsed since the last pass, so a change arriving at rest renders with
no wait. The worst case — a change arriving 1 ms after a pass — waits 99 ms. That is
worse than the spike's 5 ms bar 4 and is recorded as such rather than rounded off.
Two facts make it acceptable: the publish rate at rest is ~1–2 Hz (host metrics
resample every `METRICS_REFRESH_TICKS` = 100 ticks, countdowns tick in whole
seconds), so the case needs two changes inside one 100 ms window; and the *native*
cursor still moves in the frame the key was handled, so what waits is a
reproduction, not the highlight the user is watching. A handover changes the second
half of that, which is exactly why the ceiling is 100 ms and not 1 s.

## Why the trigger is per source rather than per publication

Nudging on any publication was the simpler design and is rejected. The snapshot is
one value covering six sections, so a session-list pane would re-render every time
host CPU resampled — a timer wearing a different hat, at the same ~1 Hz. The spike's
bar 3 (idle CPU) would stay missed for the pane it was written about.

Per-source costs a mapping table and a comparison that groups fields, both of which
the compiler checks. What it buys is that the bundled set's idle render cost is
**zero**: no bundled plugin declares `state-read`, so with nothing moving, nothing
renders.

## Why the change gate becomes the source set

`publish_pane_context` compared whole snapshots. It now publishes exactly when
`changed_sources` is non-empty. Two comparisons where one will do would be wasteful,
but the real reason is that two gates can disagree: a field belonging to no source
would publish (by inequality) and nudge nobody, and the pane would go stale with
nothing failing.

The safety is structural: `changed_sources` destructures **both** snapshots by name
with no `..` rest pattern, so adding a field to `PaneContext` fails to compile until
it is assigned to a source. That is the same device the recorded pane oracles use
(ADR-42), applied to the other end of the same data. A table-driven test then pins
the equivalence — for a list of one-field mutations, `changed_sources(a, b).is_empty()
== (a == b)` — so the two definitions of "changed" cannot drift.

## The one surviving timer

A pane whose plugin holds `state-read` can draw from `plugin_kv`, which its own
service half may write from a headless `automation tick`. Nothing on the UI thread
observes that, so there is no event to trigger on, and dropping the periodic render
would freeze such a pane until something unrelated moved.

It rides the source-file poll's existing 1 s cadence rather than a second timer:
both exist for changes the kernel cannot be told about, and one cadence is one thing
to reason about. It is raised only when a running plugin actually declares the
capability, so the bundled set pays nothing.

Rejected alternatives for this case:

| Alternative | Why not |
|---|---|
| Drop the periodic render entirely | A pane reading its own state would freeze until an unrelated source moved. Silent, and worse than the 1 s cadence it replaces |
| Give a plugin an API to request a render | It hands a plugin the demand-driven loop; ADR-V18 refused the same thing for motion frames, for the same reason |
| Watch `plugin_kv` for writes | The writer is a different process (`automation tick`), so this is `PRAGMA data_version` polling — the same 1 s poll with more machinery |
| A manifest field declaring a refresh rate | New surface for one consumer, and the capability already says what it reads |

## Rejected alternatives for the trigger itself

| Alternative | Why not |
|---|---|
| Keep the 1 s interval and shorten it | A 100 ms interval is 10× the idle VM cost for a pane that changes nothing, which is the cost the spike's bar 3 already fails |
| A dirty `AtomicBool` the worker polls between slices | The wake would cost up to one slice (100 ms) *on top of* the ceiling, and it cannot carry which source moved — so every pane renders for every change |
| A second channel for nudges, keeping the key channel as it is | `std::sync::mpsc` has no select, so the worker would need two receive arms and two disconnect paths. The existing channel already carries "something the UI thread wants the worker to do", which is what a nudge is |
| A condition variable shared with the publisher | The UI thread would take a lock the worker holds while inside a plugin VM. The channel is already the boundary and never blocks the UI thread |
| Watch the published slot from the worker (poll the `RwLock`) | Still a poll, and it would take a read lock at whatever rate it polled |
| Send the whole snapshot down the channel instead of the source set | The worker already reads the published slot when it renders; sending a clone per change duplicates the state and makes "which sources moved" a diff the worker recomputes |
| Nudge per pane from the UI thread (resolve which panes read what there) | The UI thread would need the grants, which live in the host on the worker's side. It also puts a policy decision on the render loop's thread for no gain |
| Render every pane on any nudge | See "per source rather than per publication" above |

## What the counters have to say

The non-negotiable property — a re-render producing the same tree costs no repaint —
is asserted on counters, not on timing. `PluginPane::apply` already compares before
reporting a change; what was missing is a counter that makes the comparison
observable. Two are added:

- `plugin_renders_applied` — trees the UI thread applied;
- `plugin_renders_changed` — the subset that changed a pane's tree.

The test feeds identical trees and asserts `applied` advances while `changed` does
not and `should_redraw()` stays false. This is the same shape as
`pane_context_publishes_once_while_unchanged`, which is the pattern the kernel-state
spec already requires ("observable through counters rather than asserted in prose").

The trigger's own properties (no render when nothing moved, one pass per interval
under a fast source, a pane selected only for a source it reads) are unit tests in
`plugin::render_trigger` with the clock passed in, so nothing sleeps.

## What the worker loop looks like afterwards

```text
loop {
    if stop { break }
    if the source-file poll is due {
        reload changed plugins; on a reload, send Panes and mark every pane
        if a running plugin reads its own state, mark that source
    }
    match trigger.due(now) {
        Now       => render the panes trigger.wanted(host.pane_reads()) names, send each
        Throttled => wait out the remainder, serving input
        Idle      => wait, serving input
    }
}
```

The wait is still capped at one 100 ms slice so a stop is noticed as promptly as
before, and input is still served while waiting — the property the ten-slice loop
existed for, kept without the render it was attached to.
