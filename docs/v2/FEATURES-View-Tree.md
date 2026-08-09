# Thurbox v2 — The View Tree

The rendering contract between plugins and the kernel. A plugin never draws
into a buffer; it returns a **view tree** — a plain data node graph, no
closures and no host handles — and the kernel converts it to an owned Rust
value, renders it with ratatui, applies the active theme, derives mouse
hitboxes, and routes input back as events ([ADR-V5](ARCHITECTURE.md#adr-v5)).

This is the largest surface a plugin author touches, so it is specified here in
full: the model, the node catalog, the layout algebra, styling, input routing,
scrolling, and the invariants the kernel enforces.

The catalog is deliberately **small**. Anything that is not a layout
primitive or a window onto kernel-owned state is a Luau widget, not a
node type ([ADR-V14](ARCHITECTURE.md#adr-v14)) — §3 and §4 explain where the
line falls and why.

---

## 1. Model

```text
plugin state ──render()──▶ view tree ──view/push──▶ kernel ──▶ ratatui frame
      ▲                                                              │
      └──────────── update(event) ◀──── input event ◀────────────────┘
```

Three properties define the contract:

1. **`render` is pure and synchronous.** It receives state, returns a tree. It
   may not yield, may not call host APIs, and must not have side effects.
   Anything that waits on the kernel belongs in a command or an event handler
   that dispatches back into `update`.
2. **The tree is complete.** Each push replaces the pane's previous tree
   entirely. There is no partial mutation API; the kernel diffs internally.
3. **The tree is plain data.** Tables of primitives, nothing else. Callbacks
   like `onSelect` are compiled by the `@thurbox` helpers into event
   descriptors the kernel hands back through `update`, so the kernel never
   holds a reference into the plugin's VM — which is what makes
   [C3](ARCHITECTURE.md#adr-v2)'s reload guarantee enforceable.

### Node envelope

Every node shares the same envelope:

```lua
type Node = {
    type: string,             -- node type, see §3
    id: string?,              -- stable id — required for focus, hitboxes, scroll
    style: Style?,            -- see §6
    flex: number?,            -- growth weight within the parent (default 0)
    size: (number | "auto")?, -- fixed rows/columns along the parent's axis
    hidden: boolean?,
    motion: Motion?,          -- kernel-timed animation, see §3.3
    anchor: Anchor?,          -- float against another node's rect — Layout §4
    measure: boolean?,        -- ask for this node's resolved rect back — Layout §5
    -- ...type-specific fields
}
```

`id` must be **stable across renders**. Scroll offsets, focus, hover state,
motion phase, `anchor` targets, and `measure` results are all keyed by `id`; a
tree that regenerates ids every render loses all of them.

`anchor` and `measure` are envelope fields rather than node types, so any node
can float or be measured. Both are specified in
[FEATURES-Layout.md](FEATURES-Layout.md) — [§4](FEATURES-Layout.md#4-anchors--the-overlay-layer)
and [§5](FEATURES-Layout.md#5-measurement) — because they are layout concerns,
not rendering ones.

---

## 2. Panes and roots

A plugin declares panes in its manifest and pushes one tree per pane. The
kernel gives each pane a rect; the tree fills it.

```lua
ctx.view.push("tasks", ui.box({ title = "Tasks" }, { --[[ ... ]] }))
```

The kernel owns everything outside the pane rect: borders drawn from the
theme's focus state, the pane title bar, the global footer, and the layout
solver that decides how wide the pane is. A plugin can request a `min_width` /
`min_height` in its manifest and hint at desired growth; it cannot position
itself absolutely or overlap another pane.

**Slots** determine placement. They are the *default preset* over the workspace
tree ([ADR-V23](ARCHITECTURE.md#adr-v23)) and an auto-placement hint for panes
a user's `layout.toml` does not name — so a plugin author declares a slot and
thinks no further, while a user who wants a grid or a spanning region edits the
tree ([FEATURES-Layout.md §2](FEATURES-Layout.md#2-the-workspace-tree)):

| Slot | Position | v1 equivalent |
|---|---|---|
| `left` | Left column, stacked in declaration order | Session list, automations pane |
| `center` | Central pane, tabbed when several are visible | Terminal, shell, code review |
| `right` | Right columns, in declaration order | Tasks panel, file viewer |
| `bottom` | Full-width strip above the footer | Global search |
| `overlay` | Centered modal, one at a time | Theme picker, settings, repo picker |

---

## 3. Node catalog

The catalog is **two tiers**, and the line between them has exactly one test
([ADR-V14](ARCHITECTURE.md#adr-v14)):

> **Does the kernel own the data or the clock?**
> Yes → it is a kernel node. No → it is a widget, and widgets are userland.

Everything that fails that test — tables, badges, progress bars, key-hint
rows, empty states, selectable lists — is a **Luau library**, not a node
type. See §4.

### 3.1 Tier 1 — primitives

Layout, text, style, and input. This set is deliberately small and is
**frozen**: it is the vocabulary every widget compiles down to.

| Node | Props | Notes |
|---|---|---|
| `box` | `title?`, `border?: none \| plain \| focus`, `padding?`, `direction?: vertical \| horizontal` | The workhorse container |
| `row` / `column` | children | Directional `box` shorthands |
| `spacer` | `size?` | Fixed or flexible gap |
| `scroll` | `id`, `offset?`, `revealNode?`, `onScroll?` | Scrollable viewport; kernel draws the scrollbar and handles wheel, drag, `PageUp`/`PageDown`, `Ctrl+D`/`U`. `revealNode` scrolls a descendant id into view ([FEATURES-Layout §5.1](FEATURES-Layout.md#51-node-props-for-the-concrete-cases)) |
| `text` | `content: string \| Span[]`, `wrap?`, `align?` | `Span` carries per-run style — the atom everything renders into |
| `input` | `value`, `placeholder?`, `onChange`, `onSubmit?` | Single line |
| `textarea` | `value`, `onChange`, `onSubmit?` | Multi-line, vertical cursor |

Text inputs are kernel-implemented on purpose. Plugins never receive
per-keystroke events for them, only `onChange` with the new value — which is
what makes `Ctrl+A`/`Ctrl+E`/`Ctrl+W`/`Ctrl+U`, word motions, and bracketed
paste behave identically in a third-party pane and in the session-name field.
Re-implementing that in userland would be the fastest way to make thurbox feel
inconsistent.

`scroll` is likewise kernel-owned: a hand-rolled scrollbar would diverge
visually from every other pane, and offset tracking needs to survive re-renders
keyed by node `id`.

### 3.2 Tier 2 — kernel surfaces

Nodes that exist **only** because their content, their clock, or their
emulator lives in the kernel ([ADR-V6](ARCHITECTURE.md#adr-v6)). A plugin
places and configures them; it does not supply their content through the tree.
(`surface` is the one that supplies content at all, and it does so over a
separate byte channel rather than in a pushed tree — §3.4.)

| Node | Props | Rendered from |
|---|---|---|
| `sessionTerminal` | `sessionId`, `view?: agent \| shell`, `scroll?` | Live vt100 grid via `tui-term` |
| `pty` | `command`, `args?`, `env?`, `cwd?`, `scroll?` | A process the kernel spawned, via `tui-term` (§3.4) |
| `surface` | `id`, `cols?`, `rows?`, `scroll?` | A vt100 grid the plugin writes bytes into (§3.4) |
| `diff` | `repo`, `target`, `layout?: unified \| split`, `selected?`, `onSelectLine?` | Kernel-computed git diff |
| `fileTree` | `root`, `expanded`, `selected`, `onSelect` | Kernel-walked directory tree |
| `code` | `content`, `language?` | Kernel syntax lexer + theme |
| `markdown` | `content` | Kernel markdown renderer + theme |
| `statusDot` | `state: working \| blocked \| done \| idle \| error \| unreachable` | Kernel spinner clock — the animation frame is kernel-timed |
| `sparkline` | `metric`, `window?` | Kernel metrics ring buffer |

These carry only identifiers and view options across the wire, never content.
A pane that embeds `sessionTerminal` costs the same per frame as v1's terminal
pane, because it *is* v1's terminal pane.

`code` and `markdown` are Tier 2 rather than userland for one reason each: the
syntax lexer is themed by the kernel palette, and a plugin-side markdown
renderer would produce a different look per plugin. `statusDot` is Tier 2
because its spinner must advance on the kernel's frame clock — but note that
this is a statement about the *clock*, not the data, which is why §3.3
generalizes it rather than adding a node per animation.

This tier grows only when the kernel gains a capability. It is not where new
widgets go.

### 3.3 Motion

A node may declare **motion**: a change over time the kernel evaluates on its
own frame clock ([ADR-V18](ARCHITECTURE.md#adr-v18)). The plugin pushes once.

> Summary. The normative specification — per-kind schemas, easing, phase and
> restart semantics, lease budgeting, accessibility caps, and the testing rules
> — is [FEATURES-Animation.md](FEATURES-Animation.md).

```lua
motion: {
    kind: "cycle" | "marquee" | "pulse" | "blink" | "tween",
    fps: number?,                 -- default 8, capped at 30
    pauseWhenUnfocused: boolean?, -- default false
    -- ...kind-specific fields
}?
```

| Kind | Applies to | Behavior |
|---|---|---|
| `cycle` | any node with `frames: Node[]` | Round-robins pre-supplied subtrees. The general case: spinners of your own design, typing indicators, hand-drawn frame animation |
| `marquee` | `text` | Scrolls content horizontally within the resolved rect. The kernel knows the rect, which is why this works despite §1.3's no-measurement rule |
| `pulse` / `blink` | any node | Oscillates a style token (`from`/`to`) or visibility. No content change |
| `tween` | any numeric prop | Interpolates `{ from, to, ms, ease }` — indeterminate progress, animated meters, bar growth |

Rules the kernel enforces:

1. **Motion is advisory.** A kernel that declines to animate — a `reduce_motion`
   setting, a pane over budget — renders frame 0. A plugin must never depend on
   a frame having been shown.
2. **Frames are supplied up front.** `cycle` carries its frames in the pushed
   tree, so a motion's cost is known at push time and counts against the same
   tree budget as everything else (§9). A 64-frame cap keeps that honest.
3. **The lease is per pane.** A pane holding live motion is exempt from the
   250 ms redraw floor at its declared rate; every other pane is unchanged.
   Leases drop when the pane hides, when `pauseWhenUnfocused` motion loses
   focus, or when the next pushed tree has no motion in it.
4. **Rate is capped twice** — 30 fps per pane, 30 fps aggregate across panes,
   degraded round-robin rather than summed.

5. **Re-pushing an identical motion on the same node `id` does not restart it.**
   Motion state is keyed by `(pane, id, signature)`, so a plugin that re-pushes
   on unrelated state changes keeps its animation running. Animated nodes
   therefore need a stable `id`. This rule and its edge cases are specified in
   [FEATURES-Animation.md §3](FEATURES-Animation.md#3-identity-and-phase).

`statusDot` is the degenerate case of `cycle` with a kernel-supplied frame
table, and stays a node because its *frames* are kernel-owned too.

What motion is not for: anything whose content is genuinely new per frame
(a live log, a game, a video). That is §3.4.

### 3.4 Real-time surfaces

`pty` and `surface` give a plugin a **terminal grid** instead of a tree
([ADR-V19](ARCHITECTURE.md#adr-v19)). `pty` spawns a process; `surface` gives
you an empty grid and a write channel. Both render through the `vt100` +
`tui-term` pipeline that powers `sessionTerminal`, so both cost exactly what a
session pane costs and no more.

```lua
ui.pty({ id = "doom", command = "doom", args = { "-iwad", wad },
         keyReport = "press-release", escape = "ctrl+esc" })
```

| Prop | Applies to | Meaning |
|---|---|---|
| `id` | both | Stable identity; the grid's lifetime is keyed by it |
| `command` / `args` / `env` / `cwd` | `pty` | What to spawn. Requires the `pty` capability |
| `cols` / `rows` | `surface` | Fixed grid size; omit to track the resolved rect |
| `keyReport` | both | `"press"` (default) or `"press-release"` |
| `escape` | both | Chord that releases focus back to the pane. Defaults to the kernel's terminal-escape chord |
| `scroll` | both | Expose scrollback, as `sessionTerminal` does |

**Input is sunk, not routed.** While a grid node holds focus, keystrokes are
encoded to bytes and written to the PTY directly — they do not become
`plugin/event`. Only two things are intercepted: the kernel-reserved chords
(focus cycle, quit) and the node's `escape`. A plugin therefore does not — and
cannot — sit on the keystroke path of its own embedded program, which is what
keeps input latency identical to a session pane's.

**`keyReport: "press-release"`** makes the kernel push the kitty
`REPORT_EVENT_TYPES` flag while that node is focused and pop it on blur, so
key-release events reach the program. This is scoped to the focused node on
purpose: enabling release reporting globally would change input for every agent
CLI thurbox launches. Programs that need held-key input — games, anything with
continuous movement — do not work correctly without it, and terminals that do
not implement the kitty protocol simply keep delivering presses only.

**Resize is the kernel's.** When the resolved rect changes the kernel resizes
the pty and raises `SIGWINCH`. Plugins do not set `COLUMNS`/`LINES` and should
not try; a hand-set value goes stale the first time a panel toggles.

**Lifetime follows the pane, not the VM.** A grid outlives plugin
suspension ([ADR-V15](ARCHITECTURE.md#adr-v15)) and hot reload — tabbing away
from a running program, or saving a file during plugin development, must not
kill it. It is torn down when the pane closes, when the plugin is deactivated,
or when a push no longer contains the node. A `pty` whose process exits holds
its final grid with an exit marker, mirroring `remain-on-exit`.

**Writing to a `surface`** goes over `ctx.surface.write`, a one-way call carrying
raw bytes for a node id — not `view/push`. The kernel drops frames under
backpressure rather than growing a queue, and the plugin can read the drop
count. Escape sequences are legal in this channel and only in this channel:
they are parsed by a kernel-owned emulator clipped to the pane rect, so they
can never reach the user's real terminal
([CONSTITUTION-DELTA N1](CONSTITUTION-DELTA.md#n1--the-kernel-renders-plugins-describe)).

The `pty` capability covers both. It is arbitrary code execution — `command:
"sh"` is not prevented — and is treated as full trust at the install prompt.
Its one genuine narrowing over `shell` is that output lands in a kernel grid
the plugin cannot read back.

---

## 4. Widgets are a Luau library

`@thurbox/widgets` ships the things that used to be node types, composed from
Tier 1 inside the plugin's own VM:

```lua
local w = require("@thurbox/widgets")  -- list, table, badge, keyHints, empty, progress
```

```lua
-- list() is ~60 lines of Luau over box/scroll/row/text — selection styling,
-- hover, activation events, and windowing.
local function list<T>(opts: ListOpts<T>): Node
    local rows = {}
    for i, item in opts.items do
        rows[i] = ui.row({
            id = `{opts.id}:{i}`,
            style = if i == opts.selected then { bg = "selection" } else nil,
            onPress = function() if opts.onSelect then opts.onSelect(i) end end,
        }, { opts.render(item) })
    end
    return ui.scroll(
        { id = opts.id, revealNode = `{opts.id}:{opts.selected}` },
        { ui.column({}, rows) }
    )
end
```

Why this matters more than it looks:

- **No kernel release is on the critical path of someone else's plugin.** A
  plugin author who needs a tree-table, a kanban column, or a sparkline grid
  writes one and publishes it. In the rich-catalog design they file an issue
  and wait for a thurbox release.
- **Widgets are replaceable.** A plugin may vendor its own widget module and
  ignore the bundled one, so the bundled version is a default rather than a
  mandate ([ADR-V14](ARCHITECTURE.md#adr-v14)).
- **Disagreement is forkable.** A plugin that wants different list semantics
  imports a different library instead of arguing about kernel defaults.
- **Widgets are unit-testable with no kernel.** They are pure functions from
  options to nodes.

The trade is real and worth stating: bundled plugins must all depend on the
same widget library or thurbox's own panes will drift visually. That is
enforced by convention and review for bundled plugins, and deliberately not
enforced for third-party ones.

---

## 5. Layout algebra

Layout is **flex along one axis per container**, resolved by the kernel:

1. Fixed children (`size: n`) take their size.
2. `size: "auto"` children are measured by content.
3. Remaining space is distributed among `flex: n` children proportionally.
4. Overflow clips; a `scroll` ancestor makes it scrollable instead.

```lua
ui.column({ padding = 1 }, {
    ui.text({ content = "Header", size = 1 }),
    ui.scroll({ flex = 1 }, { w.list({ items = items }) }),  -- all remaining rows
    w.keyHints({ size = 1, hints = { { "n", "new" } } }),
})
```

Within the base layer there is no absolute positioning, no z-index, and no
negative margin — siblings never overlap. Content that must float above the
flow (a dropdown, a context menu, a tooltip, a compose box pinned to a row)
declares `anchor` instead and resolves in a second pass against another node's
rect ([ADR-V22](ARCHITECTURE.md#adr-v22)); the full rules, including the
workspace tree that divides space between panes, are in
[FEATURES-Layout.md](FEATURES-Layout.md).

### Responsive rendering

The kernel passes the current pane rect in `plugin/resize` and in the render
context, so a plugin can branch on width the way v1's `compute_layout` does:

```lua
render = function(ctx, state)
    return if ctx.pane.width < 40 then compactView(state) else fullView(state)
end
```

Panes below their manifest `min_width` are hidden by the layout solver, not
squeezed — the same rule that drops v1's info panel below 120 columns.

---

## 6. Styling

Styles reference **semantic theme tokens**, never literal colors:

```lua
ui.text({ content = "3 failing", style = { fg = "danger", bold = true } })
```

| Token group | Tokens |
|---|---|
| Surface | `bg`, `panel_bg`, `border`, `border_focus` |
| Text | `fg`, `fg_muted`, `fg_dim`, `accent`, `heading` |
| Semantic | `success`, `warn`, `danger`, `info` |
| Status | `status_working`, `status_blocked`, `status_done`, `status_idle`, `status_error` |
| Diff | `diff_added`, `diff_removed`, `diff_added_bg`, `diff_removed_bg` |
| Emphasis | `selection`, `highlight`, `hover` |

Modifiers: `bold`, `italic`, `underline`, `dim`, `reversed`.

Token names are exactly v1's `ThemePalette` fields, so every one of the 36
built-in palettes and every user theme in `themes.toml` applies to third-party
panes with no plugin change. Literal colors are permitted
(`fg: "#ff0000"`) but flagged by `thurbox plugin doctor`, because a pane that
hardcodes color is broken in 35 of the 36 themes.

Theme changes arrive as `plugin/themeChanged`; a plugin that only uses tokens
needs no code to respond — the kernel re-renders its cached tree with the new
palette.

---

## 7. Focus and input routing

Focus is **kernel-owned and unique**. At most one pane holds focus; the kernel
routes input to that pane's plugin only.

```text
key press
   │
   ├─ kernel chords (focus cycle, quit, new session) ──▶ handled, never forwarded
   │
   ├─ command bound in the focused pane's context ──▶ plugin/command
   │
   └─ otherwise ──▶ plugin/event { type: "key" } to the focused pane's plugin
```

Within a pane, the plugin decides what has focus among its own nodes and
reflects that in the tree (`selected` on a list, `focused` on an input). The
kernel does not track intra-pane focus, which keeps focus state in exactly one
place per level and mirrors [ADR-V7](ARCHITECTURE.md#adr-v7).

**Keybindings stay user-owned.** A plugin's manifest `[[keybindings]]` are
*defaults*. They land in the same `keybindings.json`, the same F1 editor, and
the same conflict-resolution rules as kernel actions, scoped by
`context = "pane:<id>"`. A plugin cannot claim a chord the user has rebound,
and it cannot capture input while unfocused.

### Hit testing

The kernel derives hitboxes from the tree during layout, so plugins never do
coordinate math. A click resolves to the innermost node with an `id` and an
interaction handler, and arrives as either the declared callback event or
`{ type: "mouse", target: id }`. This is the same registry mechanism v1 uses
for `ClickAction`, generalized.

---

## 8. Scrolling

`scroll` nodes are kernel-managed: the kernel tracks the offset keyed by node
`id`, draws the scrollbar, handles the wheel, drag, `PageUp`/`PageDown`, and
`Ctrl+D`/`Ctrl+U`, and clamps to content height. A plugin only supplies
content and, optionally, reads the offset via `onScroll`.

A plugin that needs to move the viewport itself (jump to a search hit, follow
a selection) sets `offset` explicitly in the next tree. Mixing both is
well-defined: an explicit `offset` in a pushed tree always wins over the
kernel's tracked value for that render.

---

## 9. Performance rules

The view tree exists to keep the demand-driven loop intact
([ADR-V11](ARCHITECTURE.md#adr-v11)). Five rules follow:

1. **Push on change, not on frame.** Never push from a timer unless the
   content actually changes. A pushed tree marks the UI dirty and costs a
   paint.
2. **Trees are diffed by revision.** Each push carries a monotonic `revision`;
   the kernel skips the diff when the converted tree is structurally equal to
   the previous one, so a defensive push is cheap but not free — the conversion
   itself still runs.
3. **Virtualize long lists yourself.** A `list` inside a `scroll` renders all
   its items into the tree. Above ~1,000 items, slice to the visible window
   plus a margin and use `onScroll`. The kernel does not virtualize for you,
   because only the plugin knows how to page its data.
4. **Kernel surfaces are free; content is not.** Prefer `diff` and
   `sessionTerminal` over reconstructing their content in nodes.
5. **Declare motion; do not push it.** Anything that changes on a clock rather
   than on state belongs in `motion` (§3.3). Anything that changes faster than
   ~10 Hz *because its content is genuinely new* belongs in a `surface` (§3.4).
   Pushing trees is for state changes, and there is now no case where pushing
   per frame is the right answer.

Budget: a tree above **256 KB when flattened** logs a warning; above **2 MB**
the push is rejected and the pane keeps its previous tree. Both thresholds are
diagnosable through `thurbox plugin doctor`.

---

## 10. Worked example — the tasks pane

The whole v1 tasks panel (`src/ui/tasks_panel.rs`, 400+ lines of Rust plus
`InputFocus`, `Action`, `PanelAreas`, and `ClickAction` variants) as a plugin
pane:

```lua
local ui = require("@thurbox").ui        -- Tier 1 primitives
local w = require("@thurbox/widgets")    -- userland widgets

local GLYPH = { todo = "☐", in_progress = "◐", done = "☑" }

render = function(ctx, s)
    return ui.column({ padding = 1 }, {
        ui.scroll({ id = "tasks-scroll", flex = 1 }, {
            if #s.tasks == 0
                then w.empty({ message = "No tasks", hint = "n to create one" })
                else w.list({
                    id = "tasks-list",
                    selected = s.selected,
                    onSelect = function(i) return { type = "select", index = i } end,
                    onActivate = function(i) return { type = "open", id = s.tasks[i].id } end,
                    items = s.tasks,
                    render = function(t)
                        return ui.row({}, {
                            ui.text({ content = GLYPH[t.status], style = { fg = toneFor(t.status) } }),
                            ui.text({ content = t.title, flex = 1, style = { fg = "fg" } }),
                            if t.sessionId then ui.text({ content = "⇄", style = { fg = "accent" } }) else nil,
                        })
                    end,
                }),
        }),
        w.keyHints({ hints = { { "n", "new" }, { "space", "status" }, { "d", "delete" } } }),
    })
end
```

---

Note what comes from where. `scroll` and `text` are kernel primitives, so the
scrollbar, hover, click routing, theming, and the readline chords are kernel
behavior. `list`, `keyHints`, and `empty` are ordinary Luau that composes down
to `row`/`text` — replaceable, forkable, and not on the kernel's release
schedule. The plugin itself contributes the glyph table, the row
shape, and the reducer.

---

## 10b. Worked example — an embedded program

A game, `lazygit`, or `htop` as a plugin pane. The interesting part is how
little of it is rendering:

```lua
local thurbox = require("@thurbox")
local ui, w = thurbox.ui, require("@thurbox/widgets")

return thurbox.definePlugin({
    init = function(ctx)
        return { wads = findWads(ctx), running = nil, selected = 1 }
    end,

    render = function(ctx, s)
        if s.running then
            return ui.pty({
                id = `doom:{s.running}`,
                command = "doom",
                args = { "-iwad", s.running },
                keyReport = "press-release",  -- held movement needs key releases
                escape = "ctrl+esc",
            })
        end
        return ui.column({}, {
            ui.text({ content = "Select a WAD", style = { fg = "heading" } }),
            w.list({
                id = "wads",
                selected = s.selected,
                items = s.wads,
                render = function(wad) return ui.text({ content = wad.name }) end,
                onActivate = function(i)
                    return { type = "launch", wad = s.wads[i].path }
                end,
            }),
        })
    end,

    update = function(ctx, s, e)
        if e.type == "launch" then
            return { wads = s.wads, running = e.wad, selected = s.selected }
        end
        return s
    end,
})
```

The kernel owns process spawning, PTY allocation, vt100 parsing, key encoding
straight to stdin, key-release reporting while focused, `SIGWINCH` on resize,
scrollback, theming, and the grid's lifetime across suspension and reload. The
plugin owns a launcher list and one state field.

That asymmetry is the point, and it is the test of whether §3.4 is drawn
correctly. Everything a plugin adds over "just run it in a session" — WAD
discovery, save-state management, a status pill showing the current level, a
keybinding to resume — is ordinary tree and state work, and none of it sits on
the frame path.

---

## 11. Expressiveness check

The **kernel** catalog is sized by a single test: **can every v1 pane be
rebuilt with it?** If one cannot, the kernel catalog is wrong — not the pane.
Widget-tier gaps are not gaps at all: they are Luau someone writes.

| v1 surface | Kernel nodes | Widgets | Kernel gap |
|---|---|---|---|
| Session list | `scroll`, `text`, `statusDot` | `list`, tree indentation | — |
| Terminal / shell | `sessionTerminal`, `box` | `tabs` | — |
| Info panel | `column`, `text`, `sparkline` | `badge`, definition rows | — |
| File viewer | `fileTree`, `code`, `input` | find bar | — |
| Tasks | `scroll`, `text`, `markdown`, `textarea` | `list`, `keyHints`, `empty` | — |
| Automations | `scroll`, `text`, `input` | `list`, `table`, `select` | — |
| Code review | `diff`, `input`, `box` + `anchor` | files `list`, footer pills | — |
| Global search | `bottom` pane, `input`, `text` | `list`, grouped results | Cross-pane match highlighting needs a kernel-broadcast query |
| Theme / settings / repo pickers | `overlay` pane, `scroll`, `input`, `text` | `list`, `checkbox`, modal chrome | — |
| Embedded programs (lazygit, htop, a game) | `pty` | launcher `list` | — |
| Animated indicators (spinners, pulses, marquee) | `motion` | frame tables | — |

**One kernel gap** survives the audit: a kernel-broadcast search query that
lets panes highlight matches in place. It is v2.0 scope; see
[MIGRATION.md](MIGRATION.md).

The other gap this audit originally found — a compose box anchored to a diff
line — was closed generally rather than specially. `anchor`
([ADR-V22](ARCHITECTURE.md#adr-v22)) serves every pane, so the `diff.inlineAt`
child slot it would have needed no longer exists.

That every other v1 surface reduces to **eight primitives plus nine kernel
surfaces** is the evidence that the Tier 1 freeze is realistic rather than
aspirational.

---

## 12. Anti-patterns

| Don't | Why | Instead |
|---|---|---|
| Regenerate `id`s each render | Loses scroll, focus, hover | Derive ids from stable data keys |
| Push from a repeating timer | Defeats the demand-driven loop | Push when state changes; declare `motion` for anything on a clock |
| Animate by pushing a tree per frame | Pays a plugin call + conversion + diff for what the kernel can evaluate itself | `motion` (§3.3) for cosmetic motion, `surface` (§3.4) for real-time content |
| Route a `pty`'s keys through your reducer | You cannot — a focused grid sinks input to the process | Use `escape` to get focus back |
| Hardcode hex colors | Broken in 35 of 36 themes | Semantic tokens |
| Do work in `render` | Blocks the reducer and the frame | Do it in a command, dispatch the result |
| Reimplement text editing | Diverges from the readline chords everywhere else | `input` / `textarea` |
| Build your own scrollbar | Diverges from every kernel pane | `scroll` |
| Emit ANSI escapes in `text` | Escapes are stripped; may corrupt layout | `Span` styles |
| Render 50k list items | Tree-build cost, rejected above 2 MB | Virtualize with `onScroll` |
| Ask for a new kernel node type | Blocks you on a thurbox release | Compose it from Tier 1 and publish a widget |
