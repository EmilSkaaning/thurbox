# Thurbox v2 — Limitations

Every architecture buys capability by spending something else. This document
names what the v2 plugin design cannot do, so the cost is chosen rather than
discovered in Phase 2.

Limitations fall into three buckets, and the distinction matters:

| Bucket | Meaning |
|---|---|
| **By design** | Structurally impossible; fixing it means changing an ADR |
| **Impractical** | Expressible but pays a cost that makes it a bad idea |
| **Kernel gap** | Would work if the kernel grew a surface or API — a scheduling question, not an architectural one |

---

## 1. Layout

Three of the four limitations in this section were resolved after the design
review that produced [FEATURES-Layout.md](FEATURES-Layout.md). The reasoning
that once justified them is kept, because it is the reasoning the fixes had to
answer.

<a id="11-floating-elements"></a>

### 1.1 Floating elements — *resolved by [ADR-V22](ARCHITECTURE.md#adr-v22)*

> An earlier draft: "There is no absolute positioning, no z-index, and no
> negative margin. A plugin cannot render anything that floats above its own
> content" — no autocomplete dropdown, no context menu, no tooltip, no compose
> box anchored to a diff line. The general fix was named (an `anchor` node) and
> deferred, because it "requires two-pass layout and reintroduces overlap,
> which the kernel currently gets to assume never happens."

Both objections turned out to be affordable.
[Anchors](FEATURES-Layout.md#4-anchors--the-overlay-layer) resolve against
another node's rect in a **second pass paid only by trees that use them**, and
render into a per-pane overlay layer that is z-ordered positionally. The
invariant narrows rather than disappearing: the base layer still never
overlaps, and exactly one pane still holds focus, because an anchored subtree
belongs to its pane and is not a focus target.

This also **deleted a special case** — `diff`'s bespoke `inlineAt` slot, which
existed only to reproduce v1's `render_compose_inline` for one node type.

**Still limited**: an anchor cannot escape its pane's rect in 2.0, so a
dropdown at the edge of a narrow pane is clamped rather than overflowing onto
its neighbour. Escaping needs cross-pane z-ordering and a rule for what happens
when the owning pane hides mid-interaction.

<a id="12-pane-geometry"></a>

### 1.2 Pane geometry — *resolved by [ADR-V23](ARCHITECTURE.md#adr-v23)*

> An earlier draft listed six things the five-slot model could not express: a
> full-width pane spanning the list and terminal, a 2×2 dashboard across panes,
> user-draggable splits, nested splits, a header-docked pane, and runtime
> reordering.

Five of the six fall out of the
[workspace tree](FEATURES-Layout.md#2-the-workspace-tree) without a feature
each, because they are all shapes of one structure. Slots survive as the
**default preset** and as an auto-placement hint for panes the tree does not
name, so zero-config behavior is unchanged and plugin authors still just
declare `slot = "right"`.

**Still limited**: interactive resize — dragging a split border — is deferred.
The tree makes it possible (a drag writes back a `size`), but it needs border
hit regions, a persistence policy for transient drags, and keyboard
equivalents. `layout.toml` is editable in the meantime.

Unchanged: a region under its `min_width` is hidden, not squeezed, and a plugin
still cannot renegotiate its own allocation — it branches on `ctx.pane.width`.

<a id="13-measurement"></a>

### 1.3 Measurement — *mostly resolved*

> An earlier draft: "`render` is pure and synchronous with no measurement API,
> so a plugin never learns how tall its content resolved to" — no masonry, no
> precise "scroll to rendered line N", no sizing a box to wrapped content and
> positioning a sibling against it.

Two mechanisms cover most of it without making the frame two-way
([FEATURES-Layout §5](FEATURES-Layout.md#5-measurement)). The concrete cases
get exact node props — `markdown({ revealSourceLine })`, `code({ revealLine })`,
`scroll({ revealNode })` — because the kernel owns the renderer and knows the
mapping the plugin was trying to reconstruct. The general case gets opt-in
`measure: true`, returning a node's resolved rect with the **next** event
batch.

**Still limited, and structurally**: measurement is one frame late, so
single-pass content-driven layout remains impossible. A plugin can converge
over two frames; it cannot size a box to its wrapped content *and* position a
sibling against that result within the same frame. Masonry is therefore
approximate.

<a id="14-cross-pane-alignment"></a>

### 1.4 No cross-pane alignment — *by design, and staying that way*

Two sibling panes cannot share a column ruler; each computes its own widths.

The mechanism exists and is well understood — named size groups, resolving
every member to a common width. It is rejected on **value, not difficulty**: it
couples panes that are otherwise independent in the solver, it behaves
ambiguously when one member is hidden by responsive collapse, and the demand is
a single hypothetical — a table spanning two panes, which in a terminal is
almost always better as one pane.

---

## 2. Rendering

<a id="21-graphics-and-dense-cell-art"></a>

### 2.1 Graphics and dense cell art — *impractical*

A plugin can address every cell — `text` spans carry fg/bg — so half-block
pixel art, sparkline grids, and heatmaps are *possible*. The cost makes most
of them a bad idea.

Rough tree arithmetic (a styled span ≈ 100 bytes once converted to the
kernel's owned representation):

| Content | Nodes | Converted size |
|---|---|---|
| A 50-row list, 5 spans/row | ~250 | ~25 KB |
| A full-pane 2,000-line diff with syntax spans | ~20,000 | ~2 MB — rejected |
| A 100×50 half-block image | ~5,000 | ~500 KB — over the warn threshold |

The budget is 256 KB warn / 2 MB reject
([VIEW-TREE §9](FEATURES-View-Tree.md#9-performance-rules)). In-process
conversion is far cheaper per byte than the sidecar design's JSON encode, so
these thresholds have headroom they did not have before — but the binding
constraint is **rate × size**, not size: a static 500 KB image pushed once is
tolerable; the same image rebuilt and converted at 10 Hz is not.

**Escape hatch**: a `surface` node
([VIEW-TREE §3.4](FEATURES-View-Tree.md#34-real-time-surfaces)) is a
kernel-owned vt100 grid the plugin writes bytes into, and dense or fast cell art
belongs there rather than in a tree. The tree budget is for structure; the grid
is for pixels.

Not available in *either* path: **sixel, the kitty graphics protocol, and
iTerm2 inline images**. Those need escape sequences to reach the user's real
terminal, and a `surface` deliberately terminates them in the kernel's parser
([N1](CONSTITUTION-DELTA.md#n1--the-kernel-renders-plugins-describe)). Half-block
and braille cell art round-trips fine; true image protocols do not.

<a id="22-animation"></a>

### 2.2 Animation — *resolved*

Resolved by [ADR-V18](ARCHITECTURE.md#adr-v18) (declarative motion) and
[ADR-V19](ARCHITECTURE.md#adr-v19) (real-time surfaces); specified in
[FEATURES-Animation.md](FEATURES-Animation.md).

> An earlier draft of this document called animation "the sharpest single
> limitation of the design", and it was right about the symptom and wrong
> about the cause. The cost was never the animation; it was that the only way
> to express one was a view push per frame, which drags a plugin call, a tree
> conversion, and a diff along with the paint. Three of those four costs are
> incidental. The fix was to stop pushing.

Animation now splits by what is actually changing:

| What changes | Mechanism | Cost |
|---|---|---|
| A function of time over content the plugin already sent | `motion` on a node ([VIEW-TREE §3.3](FEATURES-View-Tree.md#33-motion)) | One push, then kernel-clock repaints of one pane |
| Genuinely new content per frame | `surface` / `pty` ([VIEW-TREE §3.4](FEATURES-View-Tree.md#34-real-time-surfaces)) | Exactly what a session terminal pane costs |
| New *data*, arriving faster than the eye | Throttle in the plugin and push on state change | Unchanged |

Every case the old list named is covered: a spinner of your own design, a
typing indicator, and hand-drawn frame animation are `motion: cycle`; a pulsing
progress bar is `pulse` or `tween`; marquee text is `marquee`; a game or a live
metric render at arbitrary rate is a `surface`.

**What remains limited**, and these are real:

- **Motion is a fixed vocabulary.** `cycle` / `marquee` / `pulse` / `blink` /
  `tween`, not arbitrary per-frame computation. An animation whose frames
  depend on data that arrives *during* the animation cannot be pre-supplied,
  and belongs in a `surface`.
- **Motion is advisory.** A plugin cannot know a frame was shown, cannot
  synchronize two panes' animations, and must render correctly at frame 0.
- **Rate is capped and shared.** 30 fps per pane, 30 fps aggregate, degraded
  round-robin. A pane cannot buy more by asking.
- **A `surface` is a terminal, with a terminal's ergonomics.** Cursor
  addressing and escape sequences, not a scene graph, and no theme tokens
  inside the grid — a plugin drawing there is responsible for its own colors.
- **Still no per-frame path in the tree itself.** §4.3 (no incremental view
  updates) is unchanged; the mid-band case — a live log or metric graph that is
  neither cosmetic motion nor a full grid — still pays a whole-tree push per
  update. A targeted `view/patch` is the obvious next lever and is *not* v2.0.

### 2.3 No terminal-level output — *by design*

Plugins cannot emit escape sequences to the user's terminal, so no OSC 8
hyperlinks, no OSC 52 clipboard writes, no terminal title changes, no
cursor-shape control, no bell. v1 handles the equivalents in the kernel (OSC
0/1/2 → activity title, OSC 9/777 → notifications, `links.rs` → Ctrl+Click URL
detection) and v2 keeps them there. A plugin that wants one needs a host API,
not a rendering trick.

A `surface` is not an exception to this. Escapes written there are parsed by a
kernel-owned emulator clipped to a pane rect; what reaches the real terminal is
composited cells, exactly as with tmux output. An OSC 52 sequence written into
a `surface` sets nothing.

### 2.4 Text selection is kernel-scoped — *by design*

Mouse selection is confined to a pane's bounds (`PaneBounds` in
`ui/selection.rs`) and owned by the kernel. A plugin cannot define what a
selection *means* in its pane — e.g. column-select in a table, or selecting a
logical record rather than a rectangle of characters.

---

## 3. Input

### 3.1 No per-keystroke interception in kernel text inputs — *by design*

`input` and `textarea` are kernel-implemented and emit only `onChange`. This
is what makes readline chords identical everywhere, and it costs:

- vim-modal editing inside a field
- autocomplete driven by Tab with custom semantics
- syntax highlighting while typing
- multi-cursor
- IME composition handling

**Workaround**: build a field from `text` primitives plus raw key events. That
works, and you have then reimplemented readline — inconsistently with every
other field in the app. This is a real fork in the road for anything
editor-like.

### 3.2 Focus is pane-granular — *by design*

Exactly one pane receives input. Two panes cannot both respond to keys.

v1's global search does exactly that — the strip captures every key while
*previewing* selection into the session list, tasks panel, and automations
pane. A plugin can do this across **its own** panes. It cannot do it across
another plugin's panes, which is why cross-pane search-match highlighting is
the one kernel gap the expressiveness audit left open
([VIEW-TREE §11](FEATURES-View-Tree.md#11-expressiveness-check)), scheduled in
[MIGRATION Phase 4](MIGRATION.md#phase-4--bundled-plugins-easy-first).

### 3.3 Limited mouse vocabulary — *kernel gap*

Click, scroll, drag, move, and up. Not specified for v2.0: double-click,
right-click, middle-click, modifier-qualified clicks, or gesture recognition.
Context menus are blocked on both this and §1.1.

---

## 4. Plugin model

### 4.1 Plugins cannot extend each other — *by design*

There is no plugin-to-plugin RPC, no shared memory, and no way to wrap or
patch another plugin's pane. Composition happens **only** through points the
other plugin declared (`sessionDecorations`), the kernel-brokered event bus
([BACKEND-API §8](FEATURES-Backend-API.md#8-event-bus)), or kernel state (commands,
events, storage).

The bus is deliberately the weaker primitive: fire-and-forget, opaque JSON
payloads, no request/response, and a subscriber cannot tell whether a
publisher exists. Two plugins can cooperate when both opt in, and neither can
break the other by changing an interface — which is the property direct RPC
would lose.

Not possible unless the target plugin opted in:

- adding a column to another plugin's table
- injecting a button into another plugin's footer
- wrapping another plugin's pane in your own chrome
- a shared "theme engine" plugin others query at render time

VS Code has the same limit for the same reasons. It is what keeps plugins from
breaking each other on upgrade, and it means the set of composition points is
a design surface the kernel must keep growing thoughtfully.

<a id="42-no-synchronous-host-calls-during-render---by-design"></a>

### 4.2 No synchronous host calls during render — *by design*

`render` is pure and may not call the host; every host call yields the plugin's
coroutine. So every piece of data must already be in state. A pane showing git
status for 50 sessions must pre-fetch all 50 and keep them fresh through
events — it cannot ask at paint time.

The ergonomic tax is real: much of a non-trivial plugin is cache management.

### 4.3 No incremental view updates — *by design*

Each push replaces the pane's tree entirely. There is no "append row" or
"patch node" operation. A plugin tailing a fast log must window to the visible
rows and throttle its own pushes; at 1,000 lines/second, naive pushing is
1,000 full tree rebuilds and conversions per second.

### 4.4 Reload discards state — *by design*

Hot reload respawns the process and re-runs `init()`. A plugin holding
expensive derived state (a parsed index, a warmed cache) pays full
reconstruction cost on every save during development. Persist through `ctx.kv`
if that hurts.

<a id="45-no-middleware"></a>

### 4.5 No middleware over kernel behavior — *by design*

Plugins observe, command, and *contribute*; they cannot intercept. There is no
way to:

- veto or cancel a session delete or spawn
- rewrite or remove an existing agent arg
- redirect a spawn to a different host, repo, or agent
- transform a keystroke before the kernel routes it

The one bounded exception is **spawn contributions**
([BACKEND-API §11](FEATURES-Backend-API.md#11-spawn-contributions)), which restore v1's
`[[agent_patches]]` capability: a plugin may **append** env vars and args at
spawn, fail-open under a 500 ms deadline. Append-only and veto-free is what
keeps it a contribution rather than middleware — a plugin can add to a spawn,
never take it over.

Everything else stays observational. If a genuine veto proves necessary it is
a kernel event with a response, and it should be adopted reluctantly: the
moment plugins can block kernel operations, every kernel operation inherits
every plugin's latency and failure modes.

---

## 5. Features that cannot be plugins at all

These are kernel by [ADR-V1](ARCHITECTURE.md#adr-v1), and each is something a
user will plausibly want.

One entry that used to be on this list is gone: **an embedded PTY for an
arbitrary process** is now the `pty` surface
([ADR-V19](ARCHITECTURE.md#adr-v19)), which covers `lazygit`, `htop`, a REPL, or
a game. It is noted here because it is the only limitation this document has so
far retired, and the mechanism that retired it — give the plugin a
kernel-owned grid rather than a faster tree — is the one to reach for first when
something else on this list starts hurting.

| Wanted | Why it cannot be a plugin |
|---|---|
| **A new session backend** (Docker, Kubernetes, devcontainers, zellij, a cloud sandbox) | Backends are kernel. This is the most significant one — session transport *is* thurbox's domain, and "add a backend" is the most likely third-party ambition it cannot serve |
| **Restructuring global chrome** (header, footer, status band, tab strip mechanics) | Plugins contribute `statusItems` and `tabs`; they cannot re-lay-out the frame |
| **New theme tokens** | The palette is a kernel enum; plugins consume tokens and cannot define semantic ones |
| **Shipping a theme** | Not currently a contribution point — a gap, since `themes.toml` already supports custom palettes |
| **Pre-restore startup work** | No hook runs before the kernel restores sessions |
| **A different keybinding resolution model** | Contexts, passthrough, and conflict rules are kernel |

**Native code has an out-of-process escape hatch, and it is the only one.**
[C2](ARCHITECTURE.md#adr-v2) forbids a plugin from linking a C module, which
rules out binding an existing native library in-VM. It does not rule out
*using* one: a plugin with `shell` runs it as a child through `ctx.exec` and
parses the output, and a plugin with `pty` embeds it as a live grid — the same
boundary tmux already gives every agent CLI. So "wrap a native tool" is
supported and "wrap a native *library*" is not. A plugin that genuinely needs
in-process native code is asking to be a kernel change, and should be one.

---

## 6. Cost ceilings

| Dimension | Ceiling | Note |
|---|---|---|
| Tree size | 256 KB warn, 2 MB reject | ≈ 2,600 / 20,000 nodes |
| Sustained push rate | ~10 Hz per pane before it shows | Each push = build + convert + diff + paint |
| Animated pane rate | 30 fps per pane, 30 fps aggregate | Kernel-clock repaints; no push, no conversion |
| `surface` throughput | Frames dropped under backpressure | Costs what a session terminal pane costs |
| Memory per plugin | kilobytes per Luau VM, capped by a per-VM limit | Ten active plugins are noise against v1's footprint ([ADR-V2](ARCHITECTURE.md#adr-v2)) |
| Cold start | microseconds per VM; dominated by the plugin's own `init` | Lazy activation ([ADR-V15](ARCHITECTURE.md#adr-v15)) is now an optimization, not a necessity |
| Input→paint latency | sub-millisecond added | A reducer call plus a tree conversion; no boundary crossing |
| Command deadline | 250 ms soft, 10 s hard | Longer work belongs in an automation or a session |

Numbers are design targets to be validated in Phase 1, not measurements.

---

## 7. Kernel additions we already expect to want

Naming these now keeps the Tier 1 freeze
([ADR-V14](ARCHITECTURE.md#adr-v14)) honest — the catalog stays frozen because
pressure has somewhere to go, not because nobody pushes.

| Candidate | Unblocks | Cost |
|---|---|---|
| Mouse vocabulary (double/right-click) | Context menus, richer interaction | Small |
| `themes` contribution point | Plugin-shipped palettes | Small |
| Targeted subtree updates (a patch push) | The mid-band case in §2.2 — live logs, metric graphs | Medium; §4.3 exists because of it |
| Spawn lifecycle events with responses | Pre-spawn hooks, delete veto | Reintroduces middleware — needs care |
| Backend contribution point | Third-party session transports | Large; arguably a v3 question |

---

## 8. Tripwires — when to reconsider the architecture

The alternative documented in [ADR-V13](ARCHITECTURE.md#adr-v13) (move the TUI
to TypeScript on a JS TUI toolkit, demoting Rust to a headless daemon) is not
dead; it is on hold.
These are the conditions that should reopen it, decided in advance so the
decision is evidential rather than exhausted:

1. **The kernel-gap list grows faster than it is closed** for two consecutive
 releases. That means the node catalog, not the plugin model, is the
 bottleneck — and a JS-toolkit renderer has no catalog.
2. **More than a third of plugin authors reach for §3.1's workaround**
 (hand-built text inputs). It would mean the kernel-implemented-widget
 premise does not hold in practice.
3. **Motion and `surface` do not absorb animation demand** — plugins routinely
 need per-frame *tree* updates that are neither a fixed motion kind nor
 expressible as a grid. That would mean the tree, not the clock, is the
 bottleneck.
4. **Tree conversion shows up in a perf profile** as a top-three cost at
 realistic plugin counts.

Conversely, if none of these fire within a year of v2.0, the view-tree bet has
paid and the escape hatch can be formally retired.

---

## 9. Which of these are terminal-inherent, and which are ours

Worth separating, because only the second kind is negotiable — and because the
first kind is not a cost of the *plugin model* at all.

**Inherent to being a terminal application** — no plugin architecture would fix
these, and a GUI app simply does not have them:

- §1.3's residue (measurement is one frame late, so single-pass
  content-driven layout is impossible) and §1.4 cross-pane alignment:
  consequences of a fixed cell grid and a layout solver that resolves sizes
  before content is drawn.
- §2.1's ceiling on dense cell art, and the absence of sixel/kitty/iTerm2 image
  protocols through a `surface` (§2.3).
- §3.3's mouse vocabulary — terminals report what they report.

The comparison worth making is not "a desktop app can do more" (it can, and it
pays in reach: no ssh, no tmux, no headless host) but that these costs are the
same ones v1 already pays. Nothing in the plugin model made the terminal less
capable.

**Ours, and therefore changeable** — every one of these is a scheduling
decision, and §7 is the queue:

- §3.1 kernel-owned text inputs, §3.2 pane-granular focus, §4.1 no
  plugin-to-plugin RPC, §4.3 whole-tree pushes, §4.5 no middleware.

**Already spent from that budget.** §1.1 (floating elements) and §1.2 (pane
geometry) were on this list and were paid off by
[ADR-V22](ARCHITECTURE.md#adr-v22) and [ADR-V23](ARCHITECTURE.md#adr-v23); so
was §2.2 (animation), which an earlier draft called the design's sharpest single
limitation, by [ADR-V18](ARCHITECTURE.md#adr-v18) and
[ADR-V19](ARCHITECTURE.md#adr-v19). Each was resolved by generalizing a
mechanism the kernel already needed rather than by adding a special case — the
overlay layer instead of `diff.inlineAt`, a split tree instead of a sixth slot,
a kernel-owned clock instead of a faster push path. That is the pattern to
repeat against §7's queue, and it is why the residues above are stated as
residues rather than as the original limitation.

The honest summary: the design's real spend is **§4.2 (no synchronous host
calls) and §4.3 (no incremental updates)**, which together mean a non-trivial
plugin is substantially cache management. That is the tax an out-of-process,
push-based model charges, it is the thing a fully in-language renderer
(ADR-V13's rejected alternative) would not charge, and §8's tripwires are
calibrated to notice if it turns out to be too high.
