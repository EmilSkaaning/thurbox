# Design

## 1. What `files` grants, and the much wider thing it was asked to grant

The brief for this port proposed a `files` capability that could *list a
directory and read a file's lines*, correctly observing that a filesystem-reading
binding would be the widest power granted to a plugin so far. It was rejected —
not deferred — and the reason is worth stating precisely, because it is a finding
about the pane rather than a preference about security.

**The pane needs no filesystem access.** The rows the file viewer draws are not a
directory listing. They are the flattened form of `FileViewerState.roots`, a tree
whose shape is a record of what the *user* expanded: `children = None` means "not
read yet", and the I/O happens in `activate()` when a key toggles a directory. Of
the five facts a row carries, only its name comes from disk; depth and expansion
state are the user's navigation, `matched` is the verdict of a search the kernel
runs, and the cursor's row is the keyboard's. A plugin holding `read_dir` could
therefore draw *a* file tree, but not *this* pane — its tree would be fully
expanded (or arbitrarily expanded by its own rule), with no cursor and no search,
and the equality test that is the deliverable of a Phase 4 port could not be
written at all.

So the wider grant is strictly more power for strictly less result. Three further
reasons it stays out:

- **It would be the wrong module.** `read_dir` inside a VM call is blocking,
  unbounded I/O behind the instruction budget rather than under it: a plugin that
  expanded `node_modules` would stall its render worker for as long as the
  filesystem took, and nothing in the bounds model measures that.
- **`tests/teardown_gate.rs` reserves the name.** Its `agent-config-files` row
  flips to ready when `Capability::Fs` appears, because v1's ability to place a
  file in an agent's own config dir is a filesystem power whose replacement that
  row is waiting for. Adding a filesystem binding here would advance that verdict
  as a side effect of drawing a tree. The capability is named `Files` and grants
  no I/O, so the row stays blocked — asserted by the gate.
- **Denial by absence only works if the boundary is where it looks.** The
  capability list is the install prompt. "Reads the file tree you have open in
  the file viewer" and "reads any file on your machine" are not the same
  sentence, and only one of them is what this pane does.

**What `files` therefore is.** One reader over the published snapshot, in the
shape ADR-27 established: `nodes` (basename, depth, `isDir`, `expanded`,
`matched`), `selected`, `nerdFont`. What it deliberately does **not** carry:

| Not granted | Why |
|---|---|
| any filesystem call | the snapshot is built from a tree the pane already holds; a plugin causes no I/O, ever |
| a path — absolute or relative, root or node | a row is a basename. Depth plus name lets a plugin reconstruct the tree's *shape*, which is inherent to drawing a tree; it never reveals where on disk the tree is, the user's home, or the repository's location |
| unexpanded directories | they are not in the tree; a plugin sees exactly what the user opened |
| hidden files | `read_dir_sorted` drops dotfiles before they reach the tree |
| anything outside the session's roots | the roots are the session's worktrees and additional dirs |
| the search query | a row carries the search's *verdict*. The plugin needs no matcher, so its case folding cannot drift from the kernel's, and the query — which a user may type a filename fragment or anything else into — never crosses |
| file contents, sizes, mtimes, permissions | nothing in the pane draws them |

**Alternatives rejected**

- *A `readdir`/`readfile` binding under `files`* — above.
- *Publishing each node's absolute path.* It would make the section
  self-describing and a future "open this file" command trivial. Rejected: the
  pane draws basenames, so a path is not needed to reproduce it, and a plugin
  holding absolute worktree paths is a materially larger disclosure than one
  holding names. A command that acts on a row should carry the row's *index*
  and let the kernel resolve the path — which keeps the resolution, and the
  authorisation, in the kernel.
- *Reusing `Capability::Sessions`.* The session snapshot already publishes
  `repoName` and `additionalDirNames`, so a case could be made that the file tree
  is "more of the same session". Rejected for the reason there are four state
  capabilities and not one: a pane that wants a session's name must not have to
  ask for a listing of its contents.

## 2. Where the scroll window lives, and why the list node was the shape

The tasks port left the window in the kernel and named the closure: *a list node
carrying a selected index, windowed by the kernel from the height it has.* This
change implements exactly that, and the implementation has one property worth
calling out: the window is resolved by the **same function** in both paths.
`ui::file_viewer::visible_window` was already shared by the file tree, the tasks
pane and the automation run history; `ui::plugin_pane::render_tree` now calls it
too. So the equality claim for this pane is stronger than the tasks pane's — the
plugin's tree equals the native pane's *and* the two paint the same frame at a
size where the pane scrolls, because there is one windowing rule and both sides
go through it.

The native pane still calls `visible_window` itself, for its click hitboxes and
its scrollbar. That is two calls to one pure function with identical arguments,
which is the same arrangement `row_cells` already has between the height walk and
the paint; a test asserts the hitboxes cover exactly the rows the renderer drew.

**Alternatives rejected**

- *Report the pane's resolved rect to the plugin.* The general answer, and the
  worse one, rejected for the gauge in ADR-26 and rejected again here for the
  same reasons: rendering would become width- and height-dependent, so a resize
  would have to re-enter a VM before the frame that needed it, and a plugin that
  mis-measured would produce a broken pane rather than a refused node.
- *Publish the rows already windowed.* Rejected in ADR-29 with a reason that has
  not changed: the publisher has no height (a pane's rect exists only during a
  frame), and the plugin's pane is a *different rect in the same layout*, so rows
  windowed to the native pane's height are wrong at the plugin's.
- *A second node kind for a selectable list* (`ui.scrollList`). Rejected: two
  spellings of one node would make every pane that later grows a selection
  migrate, and a plugin author would have to know in advance whether their list
  will ever have a cursor. `selected` is optional on the one list.
- *Window by accumulated child heights rather than by child index.* More general,
  and it would be right for a list of gauges or paragraphs. Rejected as
  speculative: `selected` means "this list is a list of rows", every list that
  has a cursor in thurbox is one row per child, and `visible_window` is what the
  native panes use — matching it exactly is the property that makes the frame
  equality testable. A taller child in a selected list is clipped by the bottom
  as in any list, which is documented rather than silently different.
- *Have the renderer also draw the scrollbar for a selected list.* Tempting,
  since the renderer now knows the window. Rejected: the native pane reserves the
  track *outside* the tree's rect, so a renderer that drew its own would put two
  scrollbars in the native pane. Moving the reservation into the tree changes the
  native pane's layout, which is Phase 6's business, not a reproduction's.

## 3. Why the selected row is a style role and not a colour, an emphasis, or the list's job

The file viewer draws its cursor's row with `bg(selection_bg) fg(selection_fg)`,
and the name additionally bold. Three ways to express that were considered.

- **A colour on the node.** Refused by the tree's founding constraint: no node
  may name a colour, because thurbox ships 36 palettes and eight of them are
  light.
- **Let the list node's `selected` index drive the appearance.** Attractive — the
  kernel already knows which row it is — and wrong, because thurbox's two list
  panes disagree about what a selected row looks like: the tasks pane draws it
  `accent` + bold (`Theme::selected_item()`), the file viewer draws it in the
  selection pair. An appearance inferred from the anchor would make one of the
  two panes unreproducible. That disagreement is the argument for keeping the
  appearance in the tree and the *anchor* in the list.
- **`TextStyle::selected`, resolved by the renderer** — chosen. The plugin names
  the role "this run is part of the selected row"; the theme owns both colours of
  the pair. It is documented as *replacing* the token's colour rather than
  layering, unlike `bold`/`dim`/`underline`, because a selection is a whole
  appearance and not an attribute — and because that is what the native pane does
  (a selected row's prefix drops its muted colour entirely).

One consequence accepted: `ui.text` now takes six positional arguments
(`content, style?, bold?, underline?, dim?, selected?`). That is the practical
limit of the positional form, and a seventh should convert the flags to a table
rather than continue — deliberately not done here, because introducing a second
spelling of one node in the same change that adds a flag would leave two ways to
write every run. Plugins absorb the length in one local helper, as the bundled
`tasks` and `file-viewer` panes both do.

## 4. Where `nerdFont` is published, and why it is a fact rather than a glyph

`row_marker` reads `theme::current().nerd_font_enabled` and picks between two
glyph sets. The rule ADR-29 set is *publish the rendering only when two panes must
agree about it* — and the folder glyphs have exactly one consumer in thurbox
(`src/ui/file_viewer.rs` is the only reader of `nerd_font_enabled` outside the
theme itself). So the kernel publishes the **fact** and the plugin owns both glyph
sets, which is what keeps the pane's presentation the pane's.

It rides on the `files` section rather than a section of its own, which is a
compromise stated rather than hidden: it is a display setting, not a filesystem
fact. It lives here because the file tree's markers are its only consumer today;
a second consumer should lift it to its own section under its own capability
rather than a second copy appearing.

## 5. What the port leaves out, and how it is recorded

The search sub-mode's **bar** is out of scope (see the proposal). The important
discipline is *how* that is recorded: the three missing host features are named
in `docs/PHASE4-PANE-READINESS.md` §9 (a bordered container node, a cursor
appearance, a bottom-anchored fixed-height region), and the pane's search
behaviour that *is* expressible is ported and tested. A port that had quietly
dropped the search's row emphasis too would have looked cleaner and proved less.

Two divergences remain measurable rather than argued, each pinned by its own test
in `tests/bundled_file_viewer.rs`:

| Divergence | Native pane | Plugin's copy | Cheapest closure |
|---|---|---|---|
| the search bar | a bordered block below the tree, with a cursor and a match counter | nothing; the tree still shows the search's effect | a bordered container node, a cursor appearance, a bottom-anchored region |
| the scrollbar | a reserved rightmost column with a draggable thumb | nothing | a `scrollbar` field on the list node — but the native pane reserves the track outside the tree, so this is Phase 6 work |

And one limitation of the section itself, also pinned: the published tree is the
tree **the pane has open**. `FileViewerState` is filled lazily by the pane that
owns it (on toggle, on session change, on a global-search reveal), so before the
native file viewer has ever been opened for a session the section is empty and the
plugin's copy draws `No folders`. Publishing it would otherwise mean the presence
of a plugin decides when thurbox touches the disk, which is a worse property than
an empty pane. The closure is to rebuild the state on session change rather than
on first draw.
