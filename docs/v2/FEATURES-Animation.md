# Thurbox v2 — Animation

The normative specification for **motion**: how a plugin declares something
that changes over time, and what the kernel guarantees about it.

[FEATURES-View-Tree.md §3.3](FEATURES-View-Tree.md#33-motion) is the
view-tree-facing summary; the decision and its rationale are
[ADR-V18](ARCHITECTURE.md#adr-v18). This document is where the timing,
identity, budget, accessibility, and testing rules live.

**Scope.** Motion covers *a function of time over content the plugin already
sent*. Content that is genuinely new per frame is a `surface` or `pty`
([FEATURES-View-Tree.md §3.4](FEATURES-View-Tree.md#34-real-time-surfaces)),
which is a different mechanism with different costs — see §8.

---

## 1. Model

```text
plugin: push once ──▶ tree contains motion ──▶ kernel grants a lease
                                                      │
                        kernel frame clock ───────────┤
                                                      ▼
                                       repaint that pane at the declared rate
```

The plugin never participates in a frame. It declares what should move; the
kernel evaluates the motion each time it paints, from its own clock.

Three invariants follow, and every rule in this document is a consequence of
one of them:

1. **The plugin owns the data; the kernel owns the clock.** All frames are in
   the pushed tree.
2. **Motion is advisory.** The kernel may decline. Frame 0 must be a correct,
   readable rendering on its own.
3. **Motion costs a repaint, nothing else.** No plugin call, no tree
   conversion, no diff per frame — the kernel evaluates the motion against a
   tree it already holds.

---

## 2. Declaration

```lua
motion: {
    kind: "cycle" | "marquee" | "pulse" | "blink" | "tween",
    fps: number?,                  -- default 8; clamped to [1, 30]
    pauseWhenUnfocused: boolean?,  -- default false
    -- …kind-specific
}?
```

### 2.1 `cycle`

Round-robins pre-supplied subtrees. The general case — spinners, typing
indicators, hand-drawn frame animation.

```lua
ui.box({ id = "thinking", motion = { kind = "cycle", fps = 8, frames = FRAMES } })
```

| Prop | Type | Default | Meaning |
|---|---|---|---|
| `frames` | `Node[]` | required | 2–64 subtrees, rendered in order |
| `loop` | `boolean` | `true` | `false` holds the last frame and drops the lease |
| `hold` | `number[]` | — | Per-frame dwell multipliers, for uneven timing |

### 2.2 `marquee`

Scrolls `text` horizontally within its resolved rect. This works despite the
no-measurement rule ([LIMITATIONS §1.3](LIMITATIONS.md#13-measurement)) precisely because the
kernel is the one that knows the rect.

| Prop | Type | Default | Meaning |
|---|---|---|---|
| `cps` | `number` | `6` | Cells per second |
| `gap` | `number` | `4` | Cells between the tail and the repeated head |
| `mode` | `"loop" \| "bounce" \| "once"` | `"loop"` | |
| `pauseEnds` | `number` | `800` | Milliseconds held at each end in `bounce`/`once` |

A `marquee` whose content already fits its rect **does not animate** and takes
no lease. This is checked per resolved rect, so widening a pane silently stops
the scroll.

### 2.3 `pulse` and `blink`

Oscillate a style token or visibility. No content changes.

| Prop | Type | Default | Meaning |
|---|---|---|---|
| `from` / `to` | style token | required for `pulse` | Endpoints, interpolated in RGB |
| `period` | `number` | `1200` | Milliseconds for a full cycle |
| `duty` | `number` | `0.5` | Fraction of the period spent at `to` (`blink` only) |

`blink` is **hard-capped at 2 Hz** regardless of `fps` or `period`. See §6.

### 2.4 `tween`

Interpolates a numeric prop — indeterminate progress, animated meters, bar
growth.

| Prop | Type | Default | Meaning |
|---|---|---|---|
| `prop` | `string` | required | The node prop to drive |
| `from` / `to` | `number` | required | Endpoints |
| `ms` | `number` | `400` | Duration |
| `ease` | see below | `"easeOut"` | Easing curve |
| `repeat` | `number \| "forever"` | `0` | |
| `yoyo` | `boolean` | `false` | Reverse on alternate repeats |

Easing vocabulary — deliberately small, and closed for v2.0:

`linear`, `easeIn`, `easeOut`, `easeInOut`, `stepped`

`stepped` quantizes to whole cells, which is usually what you want in a
terminal: a smooth tween across a 20-cell bar has only 20 distinguishable
states anyway, and `stepped` stops it repainting between them.

---

## 3. Identity and phase

This is the section that prevents the most likely bug in the whole system.

**Motion state is keyed by `(paneId, node id, motion signature)`**, where the
signature is the motion's kind and parameters. The kernel records a **motion
epoch** — the clock time that key was first seen — and every frame is computed
from `now − epoch`.

The rule that matters:

> **Re-pushing an identical motion on the same node `id` preserves its epoch.**
> The animation continues from where it was. It does not restart.

Without this, any plugin that re-pushes on unrelated state changes — which is
every plugin — would reset its spinner to frame 0 on every push, and a spinner
next to a 1 Hz counter would simply never advance past frame 0. Continuity is
not an optimization here; it is correctness.

A new epoch starts when, and only when:

- the node `id` changes, or
- the motion signature changes (different `kind`, `fps`, `frames`, …), or
- the pane was hidden and is shown again, or
- the plugin reloaded.

**Nodes carrying motion must have a stable `id`.** A motion on an id-less node
falls back to a structural key (its path in the tree), which is correct only
while the tree shape is stable and silently restarts when it is not. The kernel
logs this once per node per session; `thurbox plugin doctor` lists it.

### Synchronization

Two nodes with the same signature and the same epoch tick together. Nodes with
different epochs do not, and **there is no primitive to align them**. A global
phase lock would mean the kernel choosing a wake schedule for every pane at
once; the cost is not worth "two spinners in step". Plugins that want visual
coherence should give sibling nodes one shared motion on a parent rather than
one motion each.

---

## 4. Leases

A pane whose current tree contains live motion holds an **animation lease**:
that pane, and only that pane, is exempt from the 250 ms forced-redraw floor,
up to its declared rate.

| Event | Effect on the lease |
|---|---|
| Push contains motion | Granted (or retained) |
| Push contains no motion | Dropped |
| Pane hidden | Dropped |
| Pane unfocused, motion declared `pauseWhenUnfocused` | Dropped |
| `cycle` with `loop: false` reaches its last frame | Dropped |
| `tween` completes with no `repeat` | Dropped |
| Plugin suspended, faulted, or reloaded | Dropped |

Leases are per pane, never per node: a pane with six animated nodes repaints
at the highest declared rate among them, once.

### Rate budget and degradation

Two caps: **30 fps per pane** and **30 fps aggregate** across all panes. When
declared rates exceed the aggregate cap, the kernel degrades in this order:

1. The **focused pane** keeps its declared rate, up to the per-pane cap.
2. Remaining budget is distributed round-robin over the other leases in
   **ascending declared rate**, so cheap animations are served before greedy
   ones.
3. A lease that cannot be served at **≥ 4 fps** is **frozen at its current
   frame** rather than served at a rate that reads as stutter.
4. Frozen and denied leases are counted and surfaced (§7).

Degradation is deliberately not proportional. Halving everyone makes every
animation look broken; freezing the least important ones keeps the rest
correct.

---

## 5. What the kernel already animates

v1's existing animations become the reference implementations, and each must
map onto this system rather than sitting beside it:

| v1 animation | v2 form |
|---|---|
| Session-list working spinner (`SPINNER_FRAMES`, 10 braille frames at ~8 fps) | `statusDot` — a `cycle` whose frames are kernel-owned |
| Sync progress spinner | `cycle` in the status band |
| Cursor blink in a focused terminal | Kernel surface behavior; not a lease |
| Pending-spawn placeholder (`◌`, deliberately static) | No motion — the design intent was "nothing is running yet", and it stays |

`statusDot` remains a node rather than becoming a plain `cycle` because its
*frames* are kernel-owned too: a plugin should not hard-code thurbox's spinner
glyphs, and changing them should not require every plugin to re-publish.

---

## 6. Accessibility

**`reduce_motion`** (`[motion] reduce_motion` in `settings.toml`, default
`false`) suppresses every animation application-wide — plugin motion, the
session spinner, and the sync indicator alike. With it on, no lease is ever
granted and every motion renders **frame 0**. This is why frame 0 must be a
correct, readable rendering rather than a blank or a placeholder: for some
users it is the only frame.

The setting is deliberately whole-app rather than per-plugin. A user who needs
reduced motion needs it from thurbox, not from seven plugins individually.

**Flashing.** `blink` is capped at 2 Hz and `pulse` at 2 Hz for full-cell
background changes, regardless of what was declared. The guidance this
implements is the general one for photosensitive seizure risk — stay below
three flashes per second — and a plugin cannot opt out of it. A plugin that
wants attention faster than that should use color or a glyph change, not
flashing.

**Terminals do not report a reduced-motion preference**, so there is nothing to
detect and honor automatically. The setting is the whole mechanism.

---

## 7. Observability

Motion is the one thing in v2 that makes an idle thurbox wake up, so "why is
this repainting" must always have an answer:

| Surface | Shows |
|---|---|
| `thurbox plugin doctor` | Live leases per plugin: pane, node id, kind, declared vs served rate |
| Perf HUD (`F12`) | `motion_leases`, `motion_frames`, `motion_denied`, `motion_frozen` counters |
| `thurbox.log` | One warning per id-less motion node; one per denied lease class |

The perf counters are wall-clock-free `u64`s bumped at the paint path, matching
the existing `perf_*` counter tests in `src/app/acceptance.rs`.

---

## 8. Motion versus surfaces

| | `motion` | `surface` / `pty` |
|---|---|---|
| Content | Supplied up front, replayed | Generated per frame |
| Cost | One push, then repaints of one pane | Same as a session terminal pane |
| Repaint driver | Kernel frame clock, under a lease | Output arrival, via the existing redraw detection |
| Theme | Full token styling | None inside the grid — the plugin owns its colors |
| Budget | Rate-capped, degradable | Governed by output volume, not leases |

A `surface` does **not** take an animation lease. It rides the same
output-driven redraw path a session pane already uses, which is why an embedded
program at 30 fps costs what an agent producing output at 30 fps costs — no
more, and no new mechanism.

Choose by asking what changes:

- Frames are known when you push → `motion`.
- Frames depend on data arriving *during* the animation → `surface`.
- Neither, and it changes on state → just push on state change.

---

## 9. Determinism and testing

Animation must not make the acceptance suite flaky, and it does not, because
it reads the same clock the harness already controls.

- **Motion evaluates through `app::clock`** — the thread-local test clock every
  UI-thread timer already reads through. `Harness::advance` steps animation
  exactly as it steps timers and search debounce.
- **Snapshots render frame 0.** insta snapshots evaluate every motion at its
  epoch, so a pinned screen never depends on wall time. A test that wants
  frame N calls `advance` first and asserts on that.
- **Invariants for the monkey test**: a pane with no live motion never holds a
  lease; a lease never survives its pane being hidden; served rate never
  exceeds declared rate; the aggregate cap is never exceeded.
- **Conformance fixtures**: a plugin declaring 64 frames, one declaring 200 fps
  (clamped), one declaring `blink` at 30 Hz (capped to 2), one with an id-less
  motion node (warned, still renders), and one whose every push re-declares the
  same motion (must not restart — the §3 rule, asserted directly).

---

## 10. Anti-patterns

| Don't | Why | Instead |
|---|---|---|
| Push a tree per frame to animate | Pays a plugin call + tree conversion + diff for what the kernel evaluates for free | Declare `motion` |
| Omit `id` on an animated node | Structural keying restarts the animation whenever the tree shape shifts | Give it a stable id |
| Vary motion params every push | Each change starts a new epoch, so the animation restarts every time | Keep the signature constant while it should run |
| Assume a frame was displayed | Motion is advisory; `reduce_motion` and budgets both render frame 0 | Keep state in your reducer, not in the animation |
| Animate to convey state alone | Invisible under `reduce_motion` | Pair motion with a glyph or color that reads statically |
| Use `blink` for emphasis | Capped at 2 Hz and hostile to read | A `badge`, a token change, or `pulse` |
| Animate a large subtree | `cycle` frames all ship in the tree and count against its budget | Animate the smallest node that changes |
| Reach for `motion` for live data | Frames must be known at push time | `surface`, or throttle and push |
