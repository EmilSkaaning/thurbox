# Design

## 1. What the `files` capability grants after this change, and what it still refuses

Nothing changes here, and that is the finding rather than an omission. The brief
for this port expected the capability to widen; the measurement says the missing
parity is not data.

| The section carries | The section refuses | Why the refusal survives |
|---|---|---|
| a row's basename | its path, absolute or relative | a path is only needed in order to **act** on a file, and acting is a process launch (below), so the path would be a grant with no reachable use |
| its depth and expansion state | a directory's unexpanded contents | expansion is a filesystem read the *kernel* performs on a keystroke; publishing what was not expanded would make the pane's tree the publisher's rather than the user's |
| whether a running search matched it | the query text | the query is drawn only inside the search bar, which needs a frame node, a cursor cell and a bottom-anchored region — none of which exists |
| the cursor's index | a file's contents | the native pane never shows contents; it opens an external editor. Publishing them would be a read no parity requirement asks for |
| whether nerd-font glyphs are on | any filesystem handle | `Capability::Fs` stays undeclared, reserved by `tests/teardown_gate.rs` for v1's "place a file in an agent's own config dir" power |

The one-sentence form: **the file viewer's plugin is short of powers, not of
facts.** A plugin given every fact in the left column and none of the right can
already draw this pane exactly; what it cannot do is move the cursor, expand a
row, or open a file, and no additional publication changes that.

## 2. Where the scroll track is reserved, and by whom

The native pane reserved its track *outside* its tree: `render_rows` called
`scrollbar::reserve_track(list_area, …)`, painted the tree into the narrowed
rect, and drew the thumb itself. A plugin pane's renderer never called
`reserve_track` at all, so the plugin's copy had no track — divergence 2 in
`tests/bundled_file_viewer.rs`.

The chosen shape: **the list node declares that it wants a track, and the
renderer reserves and draws it.** The renderer already resolves this list's
*height* (ADR-30), so resolving the one column the track occupies is the same
trade applied to width, and it lands the drawing in exactly one place.

The native pane keeps calling `reserve_track` — but only for the numbers it must
return: the row hitboxes (which must exclude the track's column, or a click on
the thumb would select a row) and the `ScrollbarGeom` the app records as a drag
target. That is the same shape the pane already had for `visible_window`, whose
second call carries the comment explaining it: the same function, so the rows a
user can click cannot drift from the rows that were drawn.

Two supporting seams keep "one definition" true rather than approximately true:

- `scrollbar::draw_into` draws the thumb into a `Buffer`, and the pre-existing
  `render_into` becomes a thin `Frame` wrapper over it. The tree renderer has a
  buffer and no `Frame`, and a second thumb-drawing implementation is exactly
  what would let the two panes differ by a cell.
- `scrollbar::geom_for` derives the recorded geometry from a track rect without
  drawing anything, and `render_into` uses it too. So "the geometry is the
  rightmost column of the reserved track" is stated once.

### Rejected: leave the reservation in the native pane and give the renderer its own

Two call sites computing the same column, with nothing forcing them to agree.
This is the arrangement ADR-30 rejected for the scroll *window* — "two would be
two panes disagreeing about which rows sit beside the cursor" — and a track is
the same kind of thing one column over.

### Rejected: report the pane's resolved rect to the plugin so it can draw its own track

The fourth time this has been proposed and refused (ADR-26 for a gauge, ADR-29,
ADR-30 for height, ADR-31 for a diff row). It makes rendering width-dependent, so
a resize has to re-enter the VM before the frame that needs it, and a plugin that
mis-measures produces a broken pane rather than a refused node.

### Rejected: draw the track around the plugin's tree, as chrome, from `App::render_plugin_panes`

The host draws a pane's block, so it could draw a track inside it too. But the
host does not know the *content length* — how many rows the plugin's outermost
list has and which one its cursor is on are facts inside the tree, and the host
would have to walk the tree to find them, i.e. re-derive at the frame what the
node already says. Worse, it would draw a track for every plugin pane whether or
not its author wanted one, which is a layout decision taken away from the pane.

### Rejected: infer the track from the list already declaring a cursor

Tempting, because every list that scrolls has a cursor. It was refused because it
is not the same question: `ui::tasks_panel`, `ui::automations_panel` and
`ui::project_list` all draw selectable lists that overflow and **none** of them
reserves a track, so inferring one would put a scrollbar into three native panes
that deliberately have none — and it would move their frames, which is the thing
this change must not do.

### Accepted cost: a plugin pane's track is inert

The thumb reports the position of a cursor the plugin does not own, and mapping a
drag back to it would be a view write into whichever kernel cursor the plugin's
list happens to mirror — which the host cannot know. So the track is drawn and no
drag target is recorded for a plugin pane. It is honest rather than tidy: the
native pane's track is draggable, the plugin's is an indicator, and
`tests/file_viewer_pane_input_gap.rs` records it as one more consequence of the
missing view write rather than leaving it to be discovered.

### Accepted cost: a track with no cursor sits at the top

A list may declare a track and no cursor. The thumb then draws at position 0,
which is what the native pane does with a cursor on its first row. Refusing the
combination was considered and rejected: the published file section drops its
cursor when it falls past the published bound, so a plugin would have to make its
*node shape* depend on whether the kernel published a cursor — and the two trees
must be equal in exactly that case (`a tree with no cursor` is one of the
equality cases).

## 3. Why the pane's keys are not portable, in the order they fail

All seven actions dispatch into `FileViewerState`, which is view state the
kernel owns:

| Action | What it writes | The power a plugin would need |
|---|---|---|
| `FileViewerDown` / `FileViewerUp` | the cursor | a **view write** |
| `FileViewerCollapse` | the expansion set, and the cursor when it jumps to a parent | a view write |
| `FileViewerExpand` on a directory | the expansion set, **reading the directory** to fill it | a view write *or* a filesystem capability |
| `FileViewerExpand` on a file | nothing in the tree — it **launches an editor process** | a process launch, wider than any capability defined |
| `FileViewerSearch` | the query, and the expansion set as matches are revealed | a view write, plus a sub-mode |
| `FileViewerNextMatch` / `FileViewerPrevMatch` | the cursor, and the expansion set | a view write |

The tasks pane at least had two keys that needed no new host power and failed for
a second reason (ADR-38: the input path and the cursor path are disjoint). This
pane has none: there is no file-viewer key that is a record write.

The `/` sub-mode fails a *different* parity requirement — "its keys are
rebindable and appear in the F1 editor". They are not, by design:
`App::focus_key_context` returns `Global` while `search_active`, so every key in
the sub-mode is fixed. A plugin could collect the same keystrokes through
`onKey`, and they would search nothing, because the search's effect is expansion
plus cursor movement plus the per-row `matched` verdict — all of it kernel state
with no channel inward. That is the objection that stopped global search from
being ported, met again from inside a pane that *is* a pane.

### The structural fact that is new: the module is the model

For every pane ported so far, the module the teardown deletes is a renderer over
records something else owns — `tasks_panel.rs` over `Task` rows, `project_list.rs`
over sessions, `info_panel.rs` over a session's info. `file_viewer.rs` is not:
`FileViewerState` lives there, `App` owns one as a field, and
`App::build_files_snapshot` reads it. The module also owns `visible_window`,
which `ui::plugin_pane` calls to window **every plugin list** and which four
other native panes call.

So "delete the native renderer" for this pane means deleting the state the
replacement reads and the scrolling every plugin pane depends on. A handover
therefore needs the model lifted out of `ui` first — a refactor with its own
architecture question (the state is `App`'s, so `session` or `app` is its home,
and `ui` may not be reached from `session`) — which is not part of a handover and
is deliberately not attempted here.

### Rejected: lift `FileViewerState` out of `ui` now, as preparation

It would be motion without a destination. The pane cannot be handed over even
with the model moved (ADR-37's build blocker plus every row above), and a refactor
that moves a 700-line state machine between modules to enable a deletion that
remains blocked is churn whose only proof is that the tests still pass. When a
view-write channel exists and the release flip has happened, the move belongs to
that change, where the question "which module owns a pane's model" can be
answered with a consumer rather than in the abstract.

### Rejected: declare `input` and reproduce the keys against the plugin's own tree state

A plugin could hold its own cursor and its own expansion set, receive `j`/`k`,
and draw a tree that scrolls. It would be a *different* pane that happens to look
like this one: its cursor would not be the one `F3` moves, its expansion would not
be the one the global search reveals into, and it could not fill a directory
without a filesystem grant. Two file trees with two cursors in one interface is a
worse outcome than one pane whose keys stay with the kernel, and it would make
the equality test — the port's whole deliverable — impossible to write.

## 4. Why the gate is a separate test file

`tests/teardown_gate.rs` answers whether the native renderer may be deleted, and
its answer is already no for a reason unrelated to input (ADR-37, the build). If
the input blockers were folded in, the row's verdict would be unchanged and the
new probes would be dead weight there. `tests/file_viewer_pane_input_gap.rs`
mirrors `tests/global_search_pane_gap.rs` and `tests/tasks_pane_input_gap.rs`
instead: one probe per missing power, scoped to the declaration it reads, failing
with the name of the blocker whose verdict changed.

It keeps ADR-35's correction from the start: a probe that asked whether *any*
write-shaped binding existed reported the global-search row closed when
`setTaskStatus` landed. So the view-write probe here reads the binding table for
a binding that moves a **cursor or focus**, and a record write does not satisfy
it.
