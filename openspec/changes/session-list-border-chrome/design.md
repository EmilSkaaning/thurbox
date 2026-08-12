# Design

## 1. A third shape, not a fourth mechanism

`PaneChrome` grows one variant:

```rust
pub(crate) enum PaneChrome {
    Hints(&'static [(&'static str, &'static str)]),
    SearchBar(crate::ui::search_bar::SearchBar),
    StatusDots { statuses: Vec<SessionStatus>, spinner_frame: usize },
}
```

owned by `src/app/mod.rs` beside the two it joins, built by `App::pane_chrome` from
`KeyContext::SessionList`, and drawn by `App::paint_plugin_pane` on the block it already
builds. No new module, no new architecture edge, `tests/architecture_rules.rs` untouched.

The variant carries **statuses and a frame index**, not glyphs and colours. Both
resolutions live in `ui` (`status_glyph`, `status_color`) and are already shared by the
session list and the info panel, so a chrome that carried resolved cells would be a second
place a status becomes a colour — the exact drift `StyleToken::for_status` exists to
prevent. The spinner index travels with them because a dot for a working session is a
frame of the same animation the row's glyph is, and the two must not tick apart.

It is the first shape that subtracts **nothing**. `Hints` takes the frame's bottom content
row; `SearchBar` takes three rows below the frame. Dots go in cells the border already
owns, so the split in `paint_plugin_pane` runs unchanged and the pane's content area — and
therefore its row hitboxes — is bit-for-bit what it was. That is asserted rather than
argued (`border_chrome_costs_the_pane_no_content_row`), because "chrome" until now has
meant "a subtraction" and a reader will assume it still does.

## 2. The indicators are not chrome, and that is the finding

The gate's row names the dots and the `▲ N` / `▼ N` indicators together, as one thing the
frame carries. Building them as one thing was the obvious move and is wrong.

A chrome shape is resolved from kernel state **before** the pane is painted — that is what
lets `SearchBar` subtract its band from the seat first. The clipped counts do not exist
until after: they are outputs of `render_tree_rows`, which is the last thing
`paint_plugin_pane` does. A `PaneChrome::ClippedRows` would have to be constructed from
data the constructor cannot have, or the paint would have to run twice.

So they are not chrome at all. They are a fact about the host's own paint of its own
frame, in the same class as the frame's colour: the window is `ui::visible_item_window`,
the frame is `focus_block`, both are the kernel's, and a plugin is told neither. The host
draws them for **every** pane it paints, from the counts it already returned.

That is a deliberate generalisation rather than a session-list special case, and it has a
consequence worth stating: the tasks column, the automations band and the file tree gain
indicators they did not have. Making it conditional would mean "the tasks column hides
rows silently, the session list says how many" with no principle behind the difference —
and `ui::draw_clipped_indicators`'s own doc already anticipated this ("a seated plugin
pane's frame is drawn by the host, so the same painter serves a pane whose renderer is a
plugin"). It is also strictly an improvement: three panes stop hiding rows without saying
so, and a *user's* pane gets the same treatment thurbox's own do.

## 3. Rejected: publishing the dots

`SessionListSnapshot` already carries every session's status, so the plugin could draw its
own summary strip as the first row of its tree.

Rejected on two counts. It is not where the native pane draws it — the strip is on the
*border*, and a row of dots inside the frame costs a content row and moves every row below
it, which is exactly the "a handover changes which code draws the pane's content and
nothing else about the pane" rule broken. And the border is the host's surface: a pane
that could paint on its own frame could paint on any frame's title, which is a much larger
grant than this row needs, arriving as a side effect of a summary strip.

## 4. Rejected: a manifest field

`PaneDecl { border_status = true }`, or a `chrome = [...]` list — which is what the gate's
probe watches for (`nothing_declares_chrome` scans `PaneDecl` for `border`, `chrome`,
`badge`, `indicator`).

Rejected because it inverts who decides. The chrome exists because a pane declared it *is*
thurbox's session list (ADR-51), and everything else that follows from that declaration —
the keyboard, the focus rule, the hint row, the search bar — is resolved by the kernel
from the context, not asked for field by field. A manifest field would let a pane that is
nobody's reproduction request thurbox's session summary on its own frame, and there would
be nothing to put in it.

## 5. Rejected: a painter closure on the seat

`chrome: Option<Box<dyn Fn(&mut Frame, Rect)>>` would express all three shapes and any
future one in one line.

Rejected for the reason `PaneChrome` was data in the first place (ADR-53): with a closure,
"the kernel draws whatever it likes inside a plugin pane" is the rule, and there is
nothing to enumerate when asking what a seat may draw. The closed set is the constraint,
and a third variant is the cost of keeping it.

## 6. What this does not do

The native session list keeps drawing its own dots (`block.title_top`) and its own
indicators (`draw_clipped_indicators`), because it is still what thurbox draws. The
duplication is deliberate and short-lived: the handover deletes the module that holds it,
and `the_native_pane_is_still_what_thurbox_draws` in the gate is what fails if that order
is reversed.
