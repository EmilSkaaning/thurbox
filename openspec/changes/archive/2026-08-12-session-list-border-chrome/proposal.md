# The chrome a seat draws on the pane's frame, not only inside it

## Why

`tests/session_list_pane_handover_gap.rs` holds this row against the session list's
handover:

> `no-pane-chrome` — the pane's border, which carries one status dot per session
> (right-aligned in the block's top title) and `▲ N` / `▼ N` indicators when rows are
> clipped. The host draws a plugin pane's block around whatever the plugin returned, and
> nothing in the catalogue describes an overlay on that frame.

It is one of the two rows left, and both deciders it waited on are closed: the window is
the kernel's shared rule (ADR-63) and the model is the kernel's own (ADR-60). So the
counts this row is about are already computed — `render_tree_rows` returns
`clipped_above` / `clipped_below` for every pane it paints — and the dots are already
derived, once per frame, from the same statuses the rows are drawn from.

What is missing is a **place to put them**. `PaneChrome` (ADR-53, widened by ADR-58)
describes chrome the kernel draws *inside* a seat: a hint row on the frame's bottom line,
a bordered band below the frame. Both are subtractions from the pane's content area. The
session list's chrome is neither — it is painted **on the frame itself**, in cells the
border already owns, and it costs the pane no content row at all.

## What Changes

- **A third chrome shape**: `PaneChrome::StatusDots` — one dot per session, resolved from
  the statuses and the kernel's spinner frame, painted right-aligned in the pane's **top
  border**. It subtracts nothing: a pane whose seat carries it has exactly the content
  area it had.
- **The clipped-row indicators become the host's, for every pane it paints.** `▲ N` /
  `▼ N` are a fact about the *host's own* paint of its *own* frame — the window is
  `ui::visible_item_window` and the frame is `focus_block` — and a plugin is never told
  either. So `paint_plugin_pane` draws them from the counts the painter already returns,
  rather than from a declaration.
- **Three already-handed-over panes gain them**: an overflowing tasks column, automations
  band or file tree now says how many rows are off-screen, where it previously hid them
  silently. Named rather than discovered, and it is the same indicator the native session
  list has always drawn.
- **The native session list is untouched**, and is still what thurbox draws. This change
  makes the chrome *expressible*; adopting it is the handover's business.

## Non-goals

- **A manifest field.** No `border`, `chrome`, `badge` or `indicator` in `PaneDecl`. The
  chrome is the kernel's, resolved from kernel state for a pane that declared it *is* one
  of thurbox's panes — a plugin cannot ask for it and cannot suppress it.
- **A published dot.** Nothing new is added to `SessionRowSnapshot` or any other
  publication: the dots come from the same `SessionInfo` list the rows do, on the kernel
  side of the seam.
- **A general frame overlay.** Not a painter the seat invokes, not a free-form title, not
  a per-pane border string. `PaneChrome` stays a closed set of shapes, which is what keeps
  "the kernel draws whatever it likes inside a plugin pane" from becoming the rule.
- **Handing over the session list.** `src/ui/project_list.rs` still draws its own dots and
  its own indicators, and `the_native_pane_is_still_what_thurbox_draws` still passes.
- **A capability.** No `Capability` variant, no module binding, no widening of what a
  plugin may read or write.

## Gate

No new compile-time gate. `PaneChrome` and `paint_plugin_pane` are already behind the
`plugins` feature with the rest of the host; `ui::draw_clipped_indicators` is in every
build and is unchanged. Both builds are verified.
