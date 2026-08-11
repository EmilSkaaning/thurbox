# The file viewer's copy grows its scroll track; its keys cannot be ported

## Why

The file viewer was to be the next native pane *replaced* rather than
reproduced: bring the bundled `file-viewer` plugin to full parity — every key the
native pane answers — and then delete `src/ui/file_viewer.rs`. The v1 behaviour
at stake is the native file viewer (`src/ui/file_viewer.rs`, drawn from
`src/app/view.rs`, seated in the right column, toggled by
`Action::ToggleFileViewer` and gated by `[features] file_viewer`) together with
the seven `KeyContext::FileViewer` actions `src/session/keybindings.rs` declares
for it and the `/` search sub-mode those actions open.

Parity is not reachable, and this pane fails harder than the tasks pane did
(ADR-38). All seven of its rebindable actions write **view** state — the cursor,
the expansion set, the search — and the kernel-state channel is read-only by
construction. Two of them need powers no capability in the vocabulary names at
all:

- expanding a directory **reads the filesystem** (`FileViewerState::activate`
  calls `read_dir_sorted`), and `Capability::Fs` is deliberately undeclared —
  `tests/teardown_gate.rs` reserves it for v1's "place a file in an agent's own
  config dir" power, so a filesystem binding added here would advance a teardown
  verdict as a side effect of drawing a tree;
- expanding a **file** launches an external editor process
  (`file_viewer_expand` → `App::open_file_in_editor` → `launch_editor`, which
  spawns a GUI editor detached or stages a tmux popup). The widest capability the
  host defines is careful to make even *running* an automation a request the
  kernel fulfils.

And the `/` sub-mode cannot satisfy the parity bar at all. While it is active
`App::focus_key_context` returns `Global`, so **every** key in it is fixed rather
than rebindable — the F1 editor lists it under *Fixed (not rebindable)* on
purpose. A plugin declaring `input` would receive those keystrokes, but the
search's whole effect — expanding directories to reveal matches, moving the
cursor between them, marking which rows matched — is kernel state a plugin
cannot write. So a plugin-owned `/` would collect a query that searches nothing,
which is the objection that stopped global search from being ported (`§10`).

There is also a structural fact this pane is the first to have, and it changes
what "delete the native renderer" means: **`src/ui/file_viewer.rs` is the pane's
model, not only its renderer.** `FileViewerState` — the roots, the expansion
set, the cursor and the search — lives there, `App` owns one as a field, and the
published `files` section is derived from it. The module also owns
`visible_window`, the windowing rule ADR-30 gave *the plugin renderer* and which
four other native panes call. Deleting it would delete the state the plugin
reads and the scrolling every plugin list depends on.

So this change ports what can be ported, closes the pane's last rendering
divergence *inside the frame the host draws*, and makes the input verdict a gate
instead of a paragraph — because a verdict in markdown is a fact about a build
that expires without telling anyone.

The rendering gap is worth closing on its own, and it is the one
`tests/bundled_file_viewer.rs` records as **divergence 2**: the native pane
reserves its rightmost column for a scroll track and draws a thumb in it
whenever the tree overflows, and the plugin's copy has no track at all. That
test recorded the closure as "Phase 6's business" because the native pane
reserves the track *outside* the tree. Phase 6 is where this change sits, and
the recorded objection turns out to be answerable: a list that declares a track
lets the renderer reserve it from the same helper, which leaves the native
pane's rows in the same rect they were already painted into.

## What Changes

- **A list node may declare a scroll track.** When it does and the list overflows
  the rows it was given, the renderer reserves the pane's rightmost column,
  draws the thumb at the declared cursor, and lays the rows out in what is left —
  through `ui::scrollbar::reserve_track`, the helper every native pane already
  reserves with. A list that declares no track is unchanged, so no other pane
  moves.
- **The native file viewer's tree declares one**, and the native pane stops
  drawing the bar itself: it keeps computing the same reservation as *numbers*
  for its click hitboxes and its drag target, exactly as it already computes the
  same window twice for the same reason. One implementation draws the track; two
  would be two panes disagreeing about which column it occupies.
- **The bundled `file-viewer` plugin declares one**, so its copy of the pane is
  now byte-identical to the native pane inside the host's frame — asserted as
  **frame** equality at a height where the tree overflows, including the track's
  column. Divergence 2 is closed; the search bar (divergence 1) is unchanged and
  still out of scope, because it is drawn *outside* the pane's block and a plugin
  pane's block is the host's.
- **No key is declared, no pane is replaced, and no renderer is deleted.** The
  plugin stays `capabilities = ["render", "files"]` and
  `default_visible = false`; `src/ui/file_viewer.rs` stays what thurbox draws;
  the `file-viewer-plugin` teardown row stays blocked.
- **A new gate, `tests/file_viewer_pane_input_gap.rs`**, records one blocker per
  power the pane's input surface needs and cannot have, re-derived from the
  source. It keeps ADR-35's distinction (a record write is not a view write) and
  adds this pane's two of its own — the filesystem read behind an expansion and
  the process launch behind opening a file — plus the fact that the pane's model
  lives in the module a handover would delete.
- **`files` is not widened.** The brief expected it to be, and the measurement
  says the missing parity is not data: a path would only be needed in order to
  *act* on a file, and acting needs the editor-launch power rather than the
  path; contents would only be needed to preview a file, which the native pane
  does not do. The one thing the pane draws that the section withholds is the
  search **query**, and it is withheld for a reason that survives — the query is
  only drawn inside a bar the host surface cannot describe.
- **The audit records the attempt** (`docs/PHASE4-PANE-READINESS.md` §16) and
  **ADR-39** records the track's placement and the input verdict with their
  rejected alternatives.

## Capabilities

- `plugin-host/view-tree` — a list may declare a scroll track, whose column the
  kernel reserves and whose thumb the kernel draws.
- `migration/phase-4` — the file viewer's remaining in-frame divergence is
  closed; and the rule that decides when a pane's key surface may be ported gains
  this pane's two further blockers, which are powers rather than data.

## Non-goals

- **Replacing the native file viewer.** Blocked three times over: by the input
  walls above, by the model living in the module the deletion removes, and
  independently by ADR-37 — the plugin runtime is an optional dependency the
  release workflow may not enable, so a handed-over pane would be missing from
  every install. Any one alone is disqualifying.
- **Declaring `input` or any keybinding on the bundled plugin.** Every one of the
  seven actions writes view state; none is expressible as a record write, so
  unlike the tasks pane this one has not even a partial key surface to argue
  about.
- **A filesystem capability.** It would be the widest grant in the host, it is
  reserved by the teardown inventory for a different v1 power, and it is *still*
  not sufficient: a plugin holding `read_dir` could draw a file tree but not this
  pane, whose expansion state, cursor and search verdict are the user's and the
  kernel's (ADR-30's finding, unchanged).
- **Publishing the search query.** It is drawn only inside a bordered,
  bottom-anchored bar with a cursor cell — three vocabulary rows with two or
  three recorded consumers each — so publishing it now would grant a read for a
  surface no pane can draw.
- **A draggable track in a plugin pane.** The thumb a plugin's list draws reports
  a cursor the plugin does not own, so a drag has no destination: mapping one
  would be a view write, the wall this whole change is about. The track is drawn
  and no drag target is recorded, and the gate says so.
- **A view-write channel** (move a cursor, expand a row, take focus). It is the
  wall all seven keys hit and the same one global search and the tasks pane hit;
  it changes what a plugin *is*, so it is a design with its own change.

## Impact

- `src/session/view_tree.rs` — the list node's scroll-track field and its
  constructor.
- `src/plugin/view.rs`, `src/plugin/capabilities.rs`,
  `src/plugin/bundled/thurbox.d.luau` — the declaration crossing the boundary.
- `src/ui/scrollbar.rs` — the thumb becomes drawable into a buffer, and its
  geometry becomes derivable without drawing, so the renderer draws and the pane
  hit-tests from one definition of each.
- `src/ui/plugin_pane.rs` — the renderer reserves the column and draws the thumb.
- `src/ui/file_viewer.rs` — `file_tree` declares the track; `render_rows` keeps
  the reservation as numbers for its hitboxes and drag target.
- `src/plugin/bundled/file-viewer/init.luau` — one argument.
- `tests/bundled_file_viewer.rs` (divergence 2 becomes frame equality),
  `tests/file_viewer_pane_input_gap.rs` (new).
- Docs: `docs/PHASE4-PANE-READINESS.md` §16, `docs/ARCHITECTURE.md` (ADR-39),
  `docs/PHASE6-TEARDOWN-READINESS.md` (the worklist gains this pane's blockers).
- No architecture edge, no new capability, no new node kind, and no acceptance
  snapshot moves — the native pane's frames are unchanged, which is what the
  refactor has to prove.
