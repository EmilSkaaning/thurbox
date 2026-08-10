## Context

See `proposal.md` — Why. The host runs plugins and reports them; this change
lets one draw.

The constraints that decide almost everything here already exist:

- **`ui` may import `session` and `app`, never `agent` or `git`** — and, by the
  same rule, never `plugin`. `session::review` already shows the way out: the
  diff *types* live in `session`, `git` produces them, `ui` renders them.
- **The render loop is demand-driven.** It paints when `App::needs_redraw` or
  the 250 ms floor fires. A pane that dirties the UI on every tick would undo
  the idle-paint work outright.
- **A plugin VM is `!Send` and pinned to its own thread.** Nothing can call
  into a plugin from the UI thread even if it wanted to — the compiler refuses.

## Goals / Non-Goals

**Goals:**

- A contract narrow enough that the kernel can render any plugin's output
  safely, and wide enough to express the panes thurbox already has.
- Make "plugin code never runs during a frame" structural rather than careful.
- A plugin pane that is broken, slow, or hostile costs its own rectangle and
  nothing else.

**Non-Goals:**

- Input, focus, and events. A pane that responds is a much larger contract than
  a pane that displays, and mixing them would settle the input model as a side
  effect of settling the drawing model.
- Efficiency of the tree representation. Correctness and containment first;
  these trees are pane-sized.

## Decisions

### D1: View-tree types live in `session`, not `plugin`

**Decision.** `src/session/view_tree.rs` holds the node enum and the style
tokens as pure data. `plugin` converts a Lua value into it; `ui` renders it.

**Why.** It is the only placement the architecture allows without weakening a
rule: `ui` must render the tree, `ui` may import `session`, and `ui` must never
import `plugin`. It is also exactly the split `session::review` uses — types in
`session`, production in `git`, rendering in `ui` — so the shape is already
established rather than invented here.

A second benefit falls out: the renderer cannot accidentally call a plugin,
because the type it renders has no path back to a VM.

### D2: Style is a closed token set, not a color

**Decision.** A node carries an optional token from a fixed enum (`accent`,
`muted`, `danger`, `success`, `warning`, …) resolved against the active theme
at paint time.

**Why.** thurbox ships 36 palettes, eight of them light. A plugin that could
name `#1a1a1a` would be unreadable on half of them, and would silently stop
matching the moment a user switched theme. Tokens make theme-following the only
option a plugin has, which is cheaper than documenting a convention nobody
follows.

**Alternative considered.** Raw RGB with a contrast check. Rejected: it puts
the host in the business of second-guessing plugin colors, and the failure mode
(technically-passing but ugly) is worse than not offering the knob.

### D3: Render is a request/response across the existing plugin channel

**Decision.** The kernel sends a render request to the plugin's thread and
receives a converted tree back. `App` holds the last good tree per pane and
paints from it. Nothing waits.

**Why.** It reuses the channel the runtime already has, so no new concurrency
primitive enters the design, and it makes the "never during a frame" guarantee
fall out of the same `!Send` VM that already prevents UI-thread access.

Conversion from Lua happens **on the plugin's own thread**, not on the UI
thread after the fact. A malformed or pathological structure is therefore
walked, bounded, and rejected inside the sandbox that already has an
instruction budget and a memory ceiling — the depth and node limits are a
second line, not the only one.

### D4: The pane caches the last good tree and shows staleness, never blankness

**Decision.** A pane keeps its previous tree while a re-render is in flight and
when one fails, adding an error indicator on failure. Only a pane that has
never rendered shows a loading state, and only one whose *first* render failed
shows an error state.

**Why.** The alternative — clearing on failure — turns a transient plugin bug
into a flickering pane, which is both worse to look at and worse to debug. Last
good content plus an explicit indicator tells the user what is happening
without destroying what they were reading.

### D5: Dirty only on change, compared by value

**Decision.** A returned tree is compared against the current one; the UI is
marked dirty only if they differ. The node types derive `PartialEq` for exactly
this.

**Why.** The demand-driven loop's whole benefit is that an idle thurbox paints
~4 times a second instead of ~100. A pane that marked dirty on every render
response would return the app to the old behavior for anyone with a plugin
installed — the exact regression the perf work exists to prevent. Comparing
pane-sized trees is far cheaper than a paint.

### D6: Rendering is a capability, and a pane without it fails validation

**Decision.** `render` joins the capability vocabulary. A manifest that
declares a pane without requesting it is invalid.

**Why.** It keeps the manifest honest about what a plugin does — a reviewer
reading it sees "this draws" — and it lets the host refuse to hand a render
request to a plugin that never asked to draw. Rejecting the combination at
validation, rather than showing a pane that can never fill, turns a confusing
runtime state into an error that names its own fix.

### D7: `app` gains `plugin`; `ui` does not

**Decision.** The allowlist adds `plugin` to `app`'s reach. `ui` is unchanged.

**Why.** `app` is the coordinator and already imports every module; holding the
host and the cached trees is its job. Keeping `ui` free of `plugin` is what
makes D1 load-bearing rather than decorative.

## Risks / Trade-offs

- **The catalog will be too small for the second real plugin.** A frozen
  catalog is a bet. → It is deliberately the set thurbox's own panes need, and
  the first bundled plugin dogfoods it. Widening a catalog is additive;
  narrowing one after plugins exist is not, which is why it starts small.

- **A plugin can still make its pane useless** — an empty tree, one line of
  text repeated. → Out of scope by design: the host guarantees containment and
  legibility, not quality.

- **Value comparison on every render response costs something.** → Bounded by
  the same node limit that bounds the tree, and paid once per response rather
  than once per frame. The alternative costs a full paint.

- **Bounded text truncation can cut a multi-byte character.** → Truncate on a
  character boundary, not a byte index; the tests cover a multi-byte string at
  the limit.

## Migration Plan

Additive and feature-gated. No persisted state, no config change, no v1 pane
touched. Rolling back removes the pane slot and the view-tree module.

## Open Questions

- **How does a plugin ask to be re-rendered?** This change refreshes on a
  host-decided trigger. A plugin that wants to push an update (a file watcher,
  a timer) needs a mechanism that does not become a repaint firehose, and that
  belongs with the change introducing the events it would react to.
