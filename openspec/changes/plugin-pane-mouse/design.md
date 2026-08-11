# Design

See `proposal.md` — Why. This records the shape chosen and what was rejected.

## 1. The renderer reports rows; the app records them

`ui::plugin_pane::render_tree` gains a return value: the rects of the outermost
list's rows, each with its 1-based index. `App::render_plugin_panes` records them
as click targets and pushes the pane's whole-rect focus fallback after them, which
is the order every native pane already uses (`first recorded match wins`).

Rejected: **an out-parameter sink** (`&mut Vec<RowHitbox>`) threaded through the
recursion. `render_tree` recurses into every container, so the sink would be
visible at every level and each one would have to know not to write to it. A
return value from the *top* call is the same information with one place to produce
it.

Rejected: **hit-testing after the fact**, by re-walking the tree at click time with
the pane's rect. It duplicates the layout arithmetic — including the kernel's
scroll window — and a click would then resolve against a *recomputed* layout rather
than the one on screen. Native panes derive their hitboxes from what they painted
for exactly this reason.

**Why the outermost list only.** A nested list would give one click two candidate
indices, and every bundled pane's rows are its top-level list's children. The rule
also matches `ui.list`'s `selectedRow`: the number a plugin sends *out* to say where
its cursor is, is the number it gets *back* when a row is clicked. A pane with no
list has no rows, which is honest — a column of lines is not a list of rows, and
guessing otherwise would let a click select a "row" the plugin does not model.

**Indices are list-space, not screen-space.** A list that names a selected row is
windowed by the kernel (ADR-30), so the hitbox for the top visible row carries
`start + 1`, not `1`. The plugin never learns the window, so reporting a screen
position would be reporting something it cannot interpret.

## 2. A click is delivered on the key channel

`PluginKeyRequest` becomes `PluginInputRequest`, carrying either a key (with its
resolved binding, ADR-34) or a click. One channel, one bounded wait, one
consumed/unconsumed answer.

Rejected: **a second channel**. The render worker serves the existing one while it
waits out its interval; a second would double the select arms and the shutdown
paths for a message with identical timing requirements.

Rejected: **fire-and-forget delivery** (no reply). It would let the UI thread avoid
waiting, but it also loses "did the plugin consume it", which is what decides
whether a repaint is needed — and the wait is already bounded at 50 ms with a
dropped click as the failure mode.

`onClick(paneId, row)` is a separate handler from `onKey`, unlike ADR-34's decision
to fold a binding into `onKey`'s arguments. The two look inconsistent and are not:
a binding is *the same event* as the key that produced it, while a click is a
different event with no key to report. A plugin that handled clicks inside `onKey`
would need a sentinel key name, which is the shape ADR-34 rejected for bindings.

## 3. Focus names the pane it landed on

`InputFocus::PluginPane` says a plugin pane is focused, not which. `App::focused_plugin_pane`
(a `(plugin, pane)` pair) is added and consulted by `App::focusable_plugin_pane`
before it falls back to the first focusable pane, so:

- a click focuses the pane clicked, and the keys after it go there;
- the keyboard focus ring is **unchanged**: it lands on `InputFocus::PluginPane`
  once and the first focusable pane is what it selects.

That asymmetry is deliberate and recorded: making the ring visit each pane in turn
is a keyboard decision (how many stops, in what order, and what `Ctrl+H` does at
the ends) that belongs with ADR-28's picker rather than with the mouse. Before this
change the distinction could not even be expressed, because nothing named a pane.

Rejected: **a payload on `InputFocus::PluginPane`**. `InputFocus` is a `Copy` enum
compared by value in dozens of places; a `String` member churns all of them for a
distinction only the plugin host cares about.

The stale-pane case matters because a pane can vanish under the pointer (hidden,
reloaded away, its plugin stopped). The remembered pair is therefore *validated*
on every read — if it no longer names a focusable pane, the fallback applies — so
the memory can never make a key go to a pane that is not on screen.

## 4. What a click does not carry, and why the list is short

No coordinate, no rect, no width, no height, no modifier, no button, no drag, no
wheel, no hover. The geometry refusal is the model's, four times over (ADR-26,
ADR-29, ADR-30, ADR-31): a plugin that knew where it was would render
width-dependently, and a resize would have to re-enter its VM before the frame
that needs it. The rest are absent because no native pane's behaviour needs them —
`ClickAction`'s row variants carry an index and nothing else.

## 5. Module ownership, against the architecture allowlist

| New/changed | Module | Allowed |
|---|---|---|
| row hitboxes from the tree | `ui::plugin_pane` | `ui` → `session` + `app`; it returns `ui::RowHitbox`, the type native panes already return |
| `ClickAction::PluginPaneRow`, `focused_plugin_pane`, delivery | `app` | unchanged |
| `onClick` | `plugin::runtime`, `plugin::lifecycle` | unchanged |

`ui` gains no reference to `crate::plugin`: the renderer takes a `ViewNode` and
returns rects, and the pane's identity is attached by `app` when it records the
target. No allowlist entry changes.

## 6. What this leaves open

- **The wheel.** `App::scroll_pane_at` maps a wheel tick to a pane; a plugin pane
  is not one of them. It needs a scroll *model* first — a plugin's list has no
  offset the kernel owns, only a selected row it declares — so a wheel tick would
  have to become "the plugin was asked to move its cursor", which is a keyboard
  question wearing a mouse costume.
- **The scrollbar.** Still no track (PHASE4 §9), so nothing to drag.
- **The focus ring visiting each pane.** Above.
- **A bundled pane that is clickable.** No bundled plugin declares `input`, so the
  first consumer arrives with the first replacement.
