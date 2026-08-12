# Design

## Context

`tests/file_viewer_pane_input_gap.rs` holds eight rows; five are closed and record that
the power they named was **not** granted. The four that remain are decisions, and
`the_verdict_is_derived_from_the_blockers` asserts that every outstanding one is a
`Gap::Vocabulary` — nothing structural is left. This change takes those four decisions.

The capability question is answered first because it is the one that could have stopped
the handover: **it needs none.** Expanding a directory reads the filesystem and expanding
a file launches `$EDITOR`, and both stay on the kernel's side of ADR-51 — the plugin
declares that it *is* thurbox's file viewer, the kernel resolves the seven
`KeyContext::FileViewer` actions, and `FileViewerState::activate` /
`App::file_viewer_expand` run exactly as they do today. Nothing in `Capability::Files`
moves: no path, no contents, no directory listing, no query. Had the only route been an
`fs` capability or a process reach, the correct answer would have been to keep the pane
native and say so; it is not, so the pane goes.

## Goals / Non-Goals

- **Goals:** name the seat; decide what a claim does while a review owns the column;
  relocate the model and the window helper; widen seat chrome to carry the search bar;
  delete `src/ui/file_viewer.rs`.
- **Non-Goals:** any widening of what a plugin may read or do; publishing the search
  query; handing over the code review; restoring the scrollbar drag.

## Decisions

### D1 — The seat is `file-viewer`, not `right`

`PaneSlot::FileViewer → RegionId::FileViewer`, mirroring ADR-53's `tasks` seat with its
argument: the right column's occupants are drawn in a fixed order (tasks, file viewer,
then plugin columns), so a `right`-slot pane lands to the *right* of the file viewer's
position. A position within a column is part of the pane, and a handover may not move a
pane one column over.

**Rejected — reuse `right`.** It is one line cheaper and puts the pane in the wrong
place, which is a change a user notices.

### D2 — The column's second occupant **preempts** the seat; it does not share it

While `App::active_review()` is `Some`, the changed-files list owns the column: the seat
is still carved, the kernel draws `ui::code_review::render_files_list` into it, and the
plugin pane holding the seat is skipped by `render_plugin_panes`. Focus follows the
rule it already had — the ring stop is `InputFocus::ReviewFiles` while a review is open
and `InputFocus::FileViewer` otherwise.

Three properties make this the right shape rather than a special case:

- **The two never coexist.** The review's list *replaces* the file viewer in that column
  by design (it is the diff's navigation aid), so "share" has no meaning here — one of
  them is drawn.
- **It is the kernel's policy about a kernel surface.** A plugin is told nothing: no
  manifest field, no published fact, no callback. Its stored visibility is untouched, so
  closing the review restores exactly what the user had, with no keystroke.
- **The seat is still carved by "something occupies this".** `layout_for` asks for the
  column when the seat is claimed **or** a review is open — the same disjunction it had,
  with `show_file_viewer` replaced by the claim.

**Rejected — ADR-46's rule (a visible plugin pane takes the seat).** Opening a review
would draw a working-tree file tree where the changed files belong, while `Ctrl+L` landed
on `InputFocus::ReviewFiles` and `j`/`k` moved a selection in a list nobody could see.
This is precisely the row the gate refused to guess at.

**Rejected — hand the review's list a second region of its own.** A region carved beside
the file viewer's would be empty in every configuration but one, and the layout would
have to know which of the two to fill — the branch the seat model exists to avoid.

**Rejected — block on the code review's handover ("move the review's list first").** The
dependency runs the wrong way. `tests/code_review_pane_handover_gap.rs` records the
review's two seats and its capture-keyed keyboard as **structural** blockers, so that
handover is refused for reasons this change cannot close; making the file viewer wait on
it would block the file viewer indefinitely for no gain. The honest ordering is the
opposite: name the seat, make the review its preemptor, and re-verdict the review's row
with the sharper reason that produces (D7).

**Rejected — a manifest field (`preempted_by = "code-review"`).** It would ask a plugin
author to name a kernel surface they cannot see, and it would let a third-party manifest
claim precedence over one of thurbox's own surfaces.

### D3 — Seat chrome widens from a row to a band, and the search bar is a band

`App::pane_hints(context) -> Option<&'static [(&str, &str)]>` becomes
`App::pane_chrome(&self, context) -> Option<PaneChrome>` with two shapes:

```rust
enum PaneChrome {
    /// One row inside the frame, at its bottom (ADR-53).
    Hints(&'static [(&'static str, &'static str)]),
    /// A bordered band below the frame (this change).
    SearchBar(ui::search_bar::SearchBar),
}
```

`paint_plugin_pane` subtracts the band from the seat **before** drawing the frame — the
same `Length(3)` split `render_chrome` did — then paints the frame and the tree into what
remains. So the pane's frame, its content area and its row hitboxes are the native pane's,
and the bar is in the row it was always in.

Two conditions differ between the shapes, and both are the native pane's own: the hint row
appears only while the pane holds **focus**; the search bar appears whenever a search is
**running or committed**, focused or not, because a committed query keeps its counter on
screen.

**Rejected — a framed multi-row node in the view tree.** It would need three additions
(a bordered container, a cursor appearance, a fixed-height bottom-anchored region) and,
worse, a `query` field on `FilesSnapshot` — which the `no-query-write` row exists to keep
absent. The kernel owns the `/` key, so it owns the query; a plugin redrawing it would be
a second renderer for one fact, the argument ADR-53 already made for the hint row.

**Rejected — draw the bar inside the frame, like the hints.** One row cheaper in code and
it moves the bar up by one row and shrinks the tree's box. A handover may not move chrome.

**Rejected — a painter closure in the chrome slot.** "The kernel paints whatever it likes
inside a plugin pane" is the rule that would make; data keeps what a seat may draw
enumerable, which is ADR-53's reason and still holds with two shapes.

### D4 — The model goes to `app`, the window helper goes to `ui`, and `FileRow` goes

`src/ui/file_viewer.rs` is three things wearing one hat. They separate as:

| Thing | New home | Why |
|---|---|---|
| `FileNode`, `Activation`, `FileViewerState`, `enumerate_paths` | `src/app/file_viewer.rs` | `App` owns the state, and it **reads directories** |
| `visible_window` | `src/ui/mod.rs` | four `ui` surfaces window a list by it |
| `render_search_bar` + `search_title`/`truncate_left`/`split_at_cursor`/`append_cursor_spans` | `src/ui/search_bar.rs` | a painter, and the seat chrome's |
| `file_tree`, `row_node`, `row_marker`, `prefix_style`, `name_style`, `render_*` | deleted | the plugin builds the tree |
| `FileRow` | deleted | field-identical to `FileNodeSnapshot`, whose only producer it now is |

**Rejected — the model into `session`, "like `session::review`".** The parallel does not
hold: `session::review` is *pure data* about a diff, and the git that produces it lives in
`git`. `FileViewerState` calls `read_dir` in `activate`, `reveal_path` and its search
expansion, so putting it in `session` would put filesystem I/O in the layer the
architecture rules keep free of it. `app` is the coordinator and already owns the value.

**Rejected — keep `FileRow`.** Two structs with the same five fields, one converting to
the other in a `map`, is a duplication the deletion is the moment to remove.

**Rejected — `visible_window` into `ui::scrollbar`** (its usual companion). It is not a
scrollbar rule: the theme picker and the plugin-pane renderer window lists that may draw
no track.

### D5 — The scrollbar drag is lost, and named

The native pane recorded `ScrollTarget::FileViewer` from the geometry `render_rows`
resolved. `paint_plugin_pane` records no drag target — `render_tree_rows` reports row
hitboxes and not the track's rect — so the variant becomes unreachable and is deleted with
it. Wheel scrolling over the column is unaffected: `App::pane_at` resolves it from
`areas.file_viewer`, which is still the seat.

**Rejected — extend `render_tree_rows` to yield the track geometry** so the kernel could
record a drag target for a pane that declared a keyboard. It is a real option and probably
the right one eventually, but it changes the plugin-pane painter's contract for *every*
seated list pane, and a handover that also changed that contract could not claim that only
the painter of one pane changed. Recorded as a follow-up in ADR-58 rather than smuggled in.

### D6 — The rebuild moves from the paint to the tick

`render_file_viewer` rebuilt the tree for the active session as it drew. With no renderer
the rebuild has to happen where the *publication* is fed, so it moves into `tick_core`
immediately before `publish_pane_context`, gated on the pane being on screen
(`pane_keyboard_taken(KeyContext::FileViewer)`). That gate is the native behaviour: a
closed column read no directories, and a hidden pane still reads none.

It marks nothing dirty. A changed tree changes the publication, the publication nudges the
render worker, and a *changed* tree paints — the demand-driven loop's existing path.

### D7 — The code review's second-seat row is re-verdicted with a sharper reason

`no-second-seat-for-the-changed-files-list` probed, in part, that no slot named
`RegionId::FileViewer`. One does now, so the probe would flip and the row would read
"closed" — which would be false. The row stays blocked and its reason gets stronger:

> the seat exists, and this list is its **preemptor**. A plugin-drawn review would have to
> claim a seat that its own other half is the reason nobody may hold, and one plugin pane
> preempting another is a precedence no manifest can express and no host can arbitrate
> between two independently-written manifests.

Its probe changes from "no slot reaches the region" to "the seat is reached by a slot **and**
the review preempts it", which is a fact about the tree in exactly the same way.

## Risks / Trade-offs

- **A build with no plugin host has no file viewer** (`--no-default-features`), and with
  it no `InputFocus::FileViewer`. `plugins` is a default feature, so no install is in that
  position; the teardown gate fails if it ever leaves. The key reports the absence in that
  build's own words rather than doing nothing.
- **The pane arrives after the first frame** for the same reason the automations band does
  (ADR-56) — but invisibly, because it seeds hidden.
- **The search bar's condition is now evaluated per frame** from `App::pane_chrome`, one
  branch on two booleans; the native pane did the same in `render_chrome`.

## Migration Plan

Delete-and-replace in one change, as the three previous handovers did: the pane's
recordings pre-date the deletion, so the oracle keeps its evidence and only loses the edge
whose right-hand side goes. `git status tests/snapshots/` must be empty afterwards.

## Open Questions

None. The drag target (D5) is a named follow-up, not an open question.
