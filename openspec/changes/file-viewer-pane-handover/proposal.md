# The file viewer becomes the plugin, and the native renderer is deleted

## Why

The file viewer is the fourth native pane handed over, and the first whose seat has a
**second kernel occupant**. Its refusal (ADR-54) recorded four things it still needed,
and — the finding that survived it — *none* of them was a capability:

- its **keys** are the kernel's and always were (ADR-51): a pane declaring
  `key_context = "FileViewer"` is focused as `InputFocus::FileViewer`, so all seven
  scoped actions resolve and the kernel performs them against `App::file_viewer`. The
  directory read (`FileViewerState::activate`) and the editor launch
  (`App::file_viewer_expand`) stay where they are, so **no filesystem capability and no
  process reach is granted**, and the gate's three structural rows go on asserting that;
- its **reproduction** is complete — equal trees, the same painted frame when the tree
  scrolls, and the scroll track (`tests/bundled_file_viewer.rs`);
- its **oracle** is ten recordings taken from the native builder (ADR-42/48), so the
  proof outlives the builder this change deletes;
- its **toggle** and its **flag** are declarable (ADR-47: `ToggleFileViewer`,
  `file_viewer`).

What was outstanding was four decisions:

1. **`no-file-viewer-seat`** — `PaneSlot` names no seat for this column's first
   occupant.
2. **`the-column-has-a-second-kernel-occupant`** — the code review's changed-files list
   is force-shown into the same column, with its own focus and its own keys. ADR-46's
   rule (a visible plugin pane takes the seat) is the wrong rule here, and the right one
   was not written.
3. **`the-module-is-the-model-and-the-window`** — `src/ui/file_viewer.rs` is not a
   renderer that can simply go. It is the pane's **model** *and* the home of
   `visible_window`, the rule every plugin list scrolls by.
4. **`no-frame-node`** — the search bar is three rows, a border and a block cursor,
   where seat chrome (ADR-53) is one row.

## What Changes

- **`PaneSlot` grows `file-viewer`**, mapping to `RegionId::FileViewer` — the region the
  native pane occupies, left of the plugin columns and right of the tasks seat.
- **A seat may be preempted by a kernel surface, and this one is.** While a code review
  is open, the review's changed-files list owns the column: the seat is carved as
  before, the kernel draws its list into it, and the plugin pane that holds the seat is
  **not painted**. The rule is the kernel's, not a manifest's — a plugin never learns
  that it was preempted, and its stored visibility is untouched, so closing the review
  restores exactly what the user had. This is the first seat where a plugin claim does
  not simply win, and it is stated as **preemption** rather than sharing because the two
  occupants never coexist: the review's list *replaces* the file viewer in that column
  by design.
- **Seat chrome widens from a row to a band.** `App::pane_hints` becomes
  `App::pane_chrome`, and a seat's chrome may be a bordered, multi-row band **below**
  the pane's frame — which is exactly where the native pane drew its search bar. The
  query, the caret and the match counter stay kernel state; the plugin's tree is laid
  out in what remains, so the pane's content area and its row hitboxes are the ones that
  pane's content had. The `files` capability still publishes no query.
- **`src/ui/file_viewer.rs` is deleted** (1601 lines). The column is
  `src/plugin/bundled/file-viewer/init.luau`, drawn from the `file-viewer` seat, bound to
  `ToggleFileViewer`, gated by `[features] file_viewer`, declaring the `FileViewer`
  keyboard.
- **The model moves to `src/app/file_viewer.rs`** — `FileNode`, `Activation`,
  `FileViewerState` and `enumerate_paths`, unchanged. It is `App`'s state machine, and
  it reads directories, so it belongs to the coordinator rather than to `session` (which
  is pure data) or to `ui` (which is being deleted here).
- **`visible_window` moves to `src/ui/mod.rs`**, the layer's shared vocabulary — four
  surfaces in `ui` window a list by it, and after this change none of them is a file
  viewer.
- **`FileRow` is deleted.** Its five fields are `FileNodeSnapshot`'s five fields, and
  with the renderer gone its only consumer is the publication, so
  `FileViewerState::rows` yields the published row type: one type for one fact.
- **The kernel's own occupant of the seat is deleted, not switched off.**
  `App::show_file_viewer` goes with the renderer (ADR-50's rule), so the column is carved
  by the claim or by an open review, never by a flag nobody paints from.
- **The tree is rebuilt on the tick rather than in the paint.** The native renderer
  rebuilt the tree for the active session as it drew; that moves to `tick_core`, just
  before the publication it feeds, and stays gated on the pane being on screen so a
  hidden pane still reads no directory.
- **`ToggleFileViewer` reports when nothing provides the pane**, and global search's file
  scope reveals the pane through the same door — the tasks pane's rules (ADR-53),
  unchanged.
- **The oracle is rewritten against its recording.** `tests/bundled_file_viewer.rs` loses
  the `file_tree` side ADR-42 predicted it would; the ten `.snap` files become the
  expectation and are **not regenerated**.
- **The code review's second-seat row is re-verdicted, not silenced.** It said "no slot
  names `RegionId::FileViewer`". A slot does now, and the row stays **blocked** for a
  stronger reason: the seat's preemptor *is* the review's own list, so a plugin review
  would have to claim a seat it is simultaneously the reason nobody may hold — and one
  plugin pane preempting another is something no manifest can express and the host
  cannot arbitrate.
- **`tests/file_viewer_pane_input_gap.rs` is retired**, its four decisions taken and its
  three structural rows (no filesystem read, no process launch, no view write) preserved
  in ADR-58 — because none of the powers they named was granted.

## Non-goals

- **No new capability, no new binding, no new node.** `Capability::Files` still publishes
  basenames and nothing else: no path, no contents, no directory listing. Opening a file
  and launching `$EDITOR` remain the kernel's, performed on the kernel's own key. If the
  handover had needed a grant, the answer would have been to keep the native pane.
- **The search bar is not published to the plugin.** It stays kernel chrome for the
  reason ADR-53 gave for the hint row: it is one fact, and a second renderer for it would
  need the query the `files` capability deliberately withholds.
- **The pane is not shown by default.** `default_visible` stays `false`, which is what
  `show_file_viewer` initialised to.
- **The scrollbar's *drag* is lost** and named rather than discovered: a plugin pane
  records no drag target, because the painter does not report the track's geometry.
  Wheel scrolling over the column is unaffected (it is resolved from the layout, not
  from a recorded scrollbar). Making a handed-over list pane's track draggable again is
  a change to the plugin-pane painter that belongs to every such pane, not a rider on
  this one.
- **The code review is not handed over here**, and this change does not bring it closer:
  it names the seat the review's list wants and then makes that list the seat's
  preemptor.

## Impact

- Affected specs: `layout/slots` (one MODIFIED), `plugin-host/panes` (one ADDED, one
  MODIFIED), `migration/handover` (one ADDED, one MODIFIED), `migration/phase-4` (two
  ADDED). `migration/teardown` is unchanged: its rules already say what a ready row means
  and that the worked example moves with the pane it names.
- Affected code: `src/ui/file_viewer.rs` (**deleted**), `src/app/file_viewer.rs` (**new**),
  `src/ui/search_bar.rs` (**new**), `src/ui/mod.rs`, `src/ui/plugin_pane.rs`,
  `src/ui/automation_detail.rs`, `src/ui/theme_picker_modal.rs`,
  `src/session/plugin_manifest.rs`, `src/app/mod.rs`, `src/app/view.rs`,
  `src/app/key_handlers.rs`, `src/app/search.rs`, `src/app/acceptance.rs`,
  `src/plugin/bundled/file-viewer/plugin.toml`,
  `src/plugin/bundled/file-viewer/init.luau`, `tests/bundled_file_viewer.rs`,
  `tests/bundled_manifests.rs`, `tests/teardown_gate.rs`,
  `tests/code_review_pane_handover_gap.rs`, `tests/session_list_pane_handover_gap.rs`,
  `tests/file_viewer_pane_input_gap.rs` (**deleted**).
- Docs: `docs/ARCHITECTURE.md` (ADR-58), `docs/PHASE4-PANE-READINESS.md`,
  `docs/PHASE6-TEARDOWN-READINESS.md`, `CLAUDE.md`.
- No schema change, no new dependency. `settings.toml`'s `[features] file_viewer` keeps
  its name and meaning; it now gates a pane the manifest binds it to.
