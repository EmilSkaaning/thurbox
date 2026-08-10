# Design — declared motion and animation leases

## 1. Where the pieces live

```text
session/motion.rs      pure data: the declaration, its bounds, phase maths,
   (no deps)           the rate allocator, and the per-pane frame table
        │
        ├── session/view_tree.rs   ViewNode::Motion — frames are children, so
        │                          they count against the existing tree budget
        │
plugin/view.rs         Lua table ─▶ ViewNode::Motion (validation, key derivation)
        │
app/motion_state.rs    the epoch table, lease bookkeeping, GC, counters
        │              (owns the clock; the only stateful piece)
        │
ui/plugin_pane.rs      draws frame N, told which N by a FrameTable
```

The split is forced by the architecture rules and is also the right one:
`ui` may not reference `plugin`, so the frame index has to arrive as **data**.
`session::motion::FrameTable` is that data — a map from node key to frame
index, built by `app` before the paint and read by `ui` during it. The renderer
therefore cannot ask a plugin what to draw, for the same structural reason it
cannot today.

`session/motion.rs` is gated on `#[cfg(feature = "plugins")]` with the rest of
the view tree. `[motion] reduce_motion` is **not** gated: it also freezes
thurbox's own spinner, which exists in every build.

## 2. Why the frame key is stored in the node

The identity rule needs a key per animated node. Both the epoch table (in
`app`) and the renderer (in `ui`) must agree on that key, and they walk the
tree by different code paths — the renderer descends with rects, the collector
does not. Deriving the key independently on each side is a drift bug waiting to
happen: the two walks would silently disagree the first time a node type gained
a child slot.

So the key is derived **once**, at conversion, where the id and the structural
path are both in hand, and stored in the node (`ViewNode::Motion { key, .. }`).
Both consumers then read the same string. A declared `id` becomes the key
verbatim; an id-less node gets `@<path>` — its index path from the root — which
is exactly the "structural key, correct only while the tree shape is stable"
fallback FEATURES-Animation.md §3 describes.

## 3. Epoch, signature, and what "identical" means

State is keyed by `(plugin, pane, node key)` and holds `{ signature, epoch }`.
The signature is a hash of everything that changes the animation: the kind, the
frame rate, the repeat flag, and the frames themselves. On each sync:

- key present with an equal signature → **keep the epoch** (the §3 rule);
- key present with a different signature → new epoch;
- key absent from the current trees → **drop the entry**.

The drop is the leak guard and it is unconditional: a pane that stops declaring
a motion, hides, or disappears loses the state in the same pass. There is no
timeout, no LRU, and no cap — the table is exactly the animated nodes currently
on screen, so it cannot grow.

Hiding therefore *loses* phase, and re-showing starts a new epoch. That matches
FEATURES-Animation.md §3 ("the pane was hidden and is shown again" starts a new
epoch) and falls out of the same GC rather than needing a rule of its own.

## 4. The tick, and why this cannot busy-paint

`App::tick_core` calls `MotionState::sync` once per tick, after plugin renders
are applied. `sync` recomputes each visible pane's frame table and returns
whether **any table changed**. Only then is the UI marked dirty.

That is the whole demand-driven argument: the tick loop runs at ~100 Hz, but an
8 fps cycle resolves to the same frame index for ~12 consecutive ticks, so 11
of every 12 ticks compute an identical table and mark nothing. A hidden pane
holds no lease, is not evaluated, and marks nothing ever. There is no timer, no
wakeup, and no separate animation thread — motion piggybacks on the tick that
already runs.

`should_redraw` is untouched. Adding "or a motion is due" there would have been
the obvious implementation and it is the wrong one: it moves the decision to
paint into the *paint check*, where it cannot be observed by `tick_core`'s
counters and would fire again on the next iteration before the frame table had
moved.

## 5. Rate allocation

`allocate_rates` is a pure function from declared rates to served rates:

1. every rate is clamped to `[1, 30]` at conversion, so the per-pane cap is
   already applied before allocation sees it;
2. the focused pane, if it has a lease, is served in full and its rate comes
   off the aggregate budget;
3. the rest are visited in ascending declared rate, each granted
   `min(declared, remaining / remaining_count)` — max-min fairness, which is
   what "round-robin, cheap animations first" means when written down;
4. a grant that is both **degraded** and below `4` fps freezes that lease
   instead: it keeps its current frame and is counted. The floor applies to
   what was lost, not to the absolute rate — a motion that asked for 2 fps and
   got 2 fps is running exactly as declared, not stuttering.

Freezing rather than slowing is deliberate and is FEATURES-Animation.md §4's
rule: halving everyone makes every animation look broken, whereas freezing the
greediest keeps the rest correct. Ascending order means the freeze lands on
whoever asked for the most.

Serving at a rate below the declared one is expressed as a **frame-interval
stretch**, not a separate clock: the frame index is `elapsed / (1s / served)`.
A served rate that equals the declared rate is the identity case, so an
un-degraded animation runs through exactly the same maths.

## 6. Why only `cycle`

`cycle` is the general case for anything whose frames are known at push time —
a pulse is two frames, a blink is two frames, a typing indicator is four. The
kinds that are *not* expressible as frames are the ones this kernel cannot
implement honestly yet:

- `marquee` scrolls text within its **resolved rect**, and the point of it is
  that the kernel knows the rect the plugin cannot measure. The view tree has
  no rect at evaluation time — `render_tree` computes rects during the paint,
  after the frame table is built. Delivering marquee means resolving layout
  before motion, which is a layout-solver change (Phase 0's `FEATURES-Layout`
  work), not a motion change.
- `tween` drives `size`, `flex`, `padding` and `scroll.offset`. The view tree
  carries none of those props — rows split space evenly and columns stack by
  content height. There is nothing to interpolate.
- `pulse` interpolates two style tokens "in RGB", which means the kernel would
  blend palette colours per frame. That is a renderer capability
  (`ui::plugin_pane` resolves tokens at paint time and has no blend), and it is
  worth having only once themes and motion have both settled.

Shipping stubs for the other four would put four declarations in the manifest
surface that quietly render frame 0 — the same failure mode the spawn-
contribution change refused for `PATH`. An unknown kind is a conversion error
that names the kinds that exist, so a plugin author learns immediately, and
adding a kind later is additive.

`blink`'s 2 Hz photosensitivity cap has no home yet as a result. It belongs
with `blink`, and `cycle` cannot enforce it (a two-frame cycle at 8 fps is a
legal flash today, exactly as a two-frame cycle in any other terminal UI is).
This is recorded rather than silently dropped; the cap ships with the kind.

## 7. `reduce_motion` placement

`[motion] reduce_motion` is a new table rather than a `[features]` flag because
it is not a feature switch: `[features]` entries hide a surface and block its
keybinding, and a user turning one off loses the feature. Reduced motion loses
nothing — every pane still renders, at frame 0. It is also whole-app by design
(§6 of FEATURES-Animation.md): one setting, not one per plugin.

It applies live, through the existing `apply_live_settings` path, and is
mirrored onto `App` like the feature flags — it is read every frame, so it
cannot come from the write-once `settings::global()`.

Freezing thurbox's own spinner is one line in `advance_spinner_frame`, which
already returns "does this need a repaint". With reduced motion it returns
`false` and leaves the frame index where it is, so the session list keeps a
static glyph and stops requesting paints on the spinner's account. That makes
the setting honest in stable builds, where there is no plugin host at all.

## 8. Counters

Four cumulative `u64`s on `PerfCounters`, matching FEATURES-Animation.md §7:

| Counter | Bumped when |
|---|---|
| `motion_leases` | a pane's lease is granted (a new lease, not a retained one) |
| `motion_frames` | a sync resolves a different frame table — i.e. one repaint attributable to motion |
| `motion_denied` | a declared motion is suppressed (reduced motion, or its pane is hidden) |
| `motion_frozen` | a lease is frozen by the aggregate budget |

`motion_frames` is the one that matters for the exit criterion: it is exactly
the number of paints motion caused, so an acceptance test can assert that a
hidden animated pane leaves it at zero and that a visible 8 fps animation
advances it about eight times per simulated second — with no wall-clock
measurement anywhere.

## 9. Rejected alternatives

- **A `motion` field on every node variant.** `ViewNode` is a flat enum with no
  common envelope, so this means adding two fields to six variants and touching
  every match. A wrapper variant carries the same information, keeps frames as
  children (so the existing node-count and depth bounds apply unchanged), and
  is what the conversion produces from the documented `motion = { … }` field —
  the Lua-facing surface is the one FEATURES-Animation.md specifies either way.
- **A dedicated animation thread or timer.** It would need to wake the render
  loop, which means a channel and a wakeup the loop must poll, for behaviour
  the existing tick already provides at 100 Hz — ten times the fastest rate any
  lease may hold.
- **Keying motion state by node pointer or tree position only.** Position-only
  keying restarts every animation whenever the tree shape shifts, which is the
  §3 bug in its purest form. Position remains the *fallback* for an id-less
  node, and the host records when it is used.
- **Letting `should_redraw` ask for the motion deadline.** §4 above.
