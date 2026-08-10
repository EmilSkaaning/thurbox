# Design — the tasks pane as a bundled plugin

## 1. What the pane is made of, and who owns each part

Reading `src/ui/tasks_panel.rs` line by line — the discipline
`docs/PHASE4-PANE-READINESS.md` §6 says finds the real gaps, rather than reading
the node catalogue — the pane is seven decisions:

| Decision | Native today | Who owns it after this change |
|---|---|---|
| status → glyph `☐ ◐ ☑` | `status_glyph` | the **plugin** (and the native tree builder) |
| status → colour | `status_color` | the **plugin**, as a style token |
| selected > dimmed > status precedence | `highlight::row_base_style` | the **plugin** |
| matched runs emphasised | `highlight::highlighted_spans_owned` | the **plugin**, from published byte offsets |
| trailing `⇄` for a live session | `LINKED_MARKER` | the **plugin** |
| empty-state hint | `render_task_list` | the **plugin** |
| which rows, how wide, how many | `visible_window` + `truncate_ellipsis` | the **kernel** — a plugin has no width and no height |

The first six are presentation and they are exactly what the port has to prove a
plugin can own. The seventh is geometry, and §4 already decided how geometry is
handled: the kernel resolves it, because reporting a resolved rect back into a VM
makes rendering width-dependent and a plugin that mis-measures produces a broken
pane rather than a refused node.

## 2. The state channel: a section on the existing snapshot

ADR-27's `session::pane_context` is the mechanism and this change adds a section
to it rather than inventing a second one. `PaneContext` gains

```rust
pub struct TaskSnapshot {
    pub title: String,
    pub status: &'static str,        // "todo" | "in_progress" | "done"
    pub selected: bool,
    pub dimmed: bool,
    pub linked: bool,
    pub match_positions: Vec<usize>,
}
pub struct TasksSnapshot { pub entries: Vec<TaskSnapshot>, pub focused: bool }
```

read by `thurbox.tasks()` under `Capability::Tasks`.

**What the kernel resolves, and why each one is not the plugin's to compute.**
`pane_context`'s existing rule is that the kernel resolves what a sandboxed
plugin *cannot* compute and publishes everything else raw. Applied here:

- `selected` — thurbox owns the keyboard. The selection moves on `j`/`k` in a
  pane the plugin does not receive keys for, and it is *also* moved by a global
  search preview; a plugin cannot know either. It is published per entry rather
  than as an index because an index would have to be reconciled against a window
  the plugin did not compute.
- `dimmed` and `match_positions` — the global search runs in the kernel over the
  kernel's fuzzy matcher. Byte offsets rather than pre-split runs, so the plugin
  still does the segmentation and the emphasis: that is presentation.
- `linked` — "does this task have an open related session" is a join across
  records, the same class of thing as `SessionSnapshot::parent_name`.
- `status` — a wire name, not a glyph and not a token. This is the deliberate
  opposite of `StatusSnapshot`, which publishes a session status's glyph *and*
  token: that mapping exists once in the kernel (`StyleToken::for_status`) and is
  shared by two native panes, so a plugin re-deriving it would be a second
  unchecked copy. A *task* status's glyph and colour live nowhere but in the
  tasks pane, so publishing them would hand the plugin the very thing the port
  is trying to prove it can own.

**Why `focused` is on the section.** The empty-state line reads `no tasks — n to
add` when the pane is focused and `no tasks` when it is not, so it is content,
not chrome. Nothing else in the tree depends on it — the selected-row style
folds focus into the per-entry `selected`.

### Rejected: read the tasks out of SQLite through a `PluginStore`-shaped seam

The obvious reading of "the tasks live in `src/storage/tasks.rs` and `plugin` may
not import `storage`" is to declare a `TaskReader` trait in `session`, implement
it in `storage`, and hand a plugin VM a factory the way `PluginStore` is handed
one. Rejected, for three reasons, and the first is decisive:

1. **It reads the wrong thing.** Four of the six facts a row draws — `selected`,
   `dimmed`, `match_positions`, `linked` — are not in the database. They are
   view state in `App`. A database seam would give the plugin the task titles and
   none of what makes a row look the way it looks, so the pane could not be
   reproduced through it.
2. **It puts a query on the render path.** `PluginStore` exists because a plugin's
   *own* key/value writes must be durable; a pane rendering once per worker cycle
   would open a connection and run a `SELECT` per render, where the snapshot is a
   clone of data the UI thread already has in hand.
3. **It would be a second state channel.** ADR-27 chose the published snapshot
   for kernel state deliberately, and having two mechanisms for "kernel state a
   pane reads" is how the two come to disagree about freshness and gating.

The `PluginStore` seam stays the pattern for a plugin's own durable state, which
is what it was designed for.

### Rejected: publish the rows the pane would draw, already fitted and windowed

This is tempting because it would make the plugin's tree *equal* the native
pane's in every case, including a narrow column — `AutomationSnapshot::label` is
already published pre-truncated, so there is precedent for publishing a fitted
string. Rejected on two counts:

- **It needs a geometry the publisher does not have.** The snapshot is built on
  the tick, in Model code that draws nothing; the pane's resolved rect exists only
  during the frame. The publisher would have to read the *previous* frame's
  width, and answer the question "what width, when the native pane is hidden?" —
  which it is by default — with an arbitrary constant.
- **It couples the plugin's pane to the native pane's size.** The plugin's pane
  is a different rect in the same layout. Fitting its rows to another pane's
  width means the plugin's copy is truncated where it has room, and windowed to
  someone else's height. A pane that renders its own rows badly at its own size
  is worse evidence than a pane that renders them plainly.

So the snapshot publishes each row's title as the model knows it, and the
divergence at a narrow width is recorded (§5) rather than papered over.

## 3. Two emphasis flags on the view tree

`TextStyle` carries `token` and `bold`. A selectable row needs three styles and
two of them are unreachable:

| Row state | Native style | Expressible before? |
|---|---|---|
| selected | `Theme::selected_item()` = accent + bold | yes — `{accent, bold}` |
| dimmed (search non-match) | `text_muted` + `DIM` | **no** |
| matched run | accent + `BOLD｜UNDERLINED` | **no** |

So `TextStyle` gains `dim: bool` and `underline: bool`, a `text` node gains the
matching optional fields, and `ui.text` gains two more positional flags. They are
ordinary text attributes in the same family as `bold`, they name no colour, and
they cost one `add_modifier` each in the renderer.

The flags are the honest closure rather than a convenience: without them
**no** plugin can draw a list with a search in it, in any pane, and every
remaining Phase 4 pane is such a list.

### Rejected: a `selected` / `matched` semantic flag instead

A row could declare "I am the selected row" and let the kernel pick the styling,
which is how `StyleToken::for_status` handles a session status. Rejected: the
kernel would then own the *look* of a plugin's list rows, so a plugin could not
build a list that looks like anything else — and thurbox's own three list panes
would still have to agree with it by hand. `dim`/`underline` keep the tree
describing what to draw, with the theme still choosing every colour.

### Rejected: a flags table as the third argument to `ui.text`

`ui.text(content, style, { bold = true, underline = true })` reads better than
five positional arguments. Rejected because it would be a second spelling of the
same node — the existing `bold` boolean cannot go away without breaking every
plugin and every bundled pane — and two ways to say one thing in the authoring
API is worse than one ugly way. The node's own fields stay the canonical form;
the positional constructor mirrors them in declaration order.

## 4. The native pane renders through the tree

The info panel's port established that a pane's tree is only evidence if the
pane *draws* it, so `ui::tasks_panel` splits the same way `ui::info_panel` did:

- `visible_rows(state, width, height) -> VisibleRows` — the geometry step. Windows
  the entries around the selection, fits each title to the column reserving room
  for the `⇄`, and resolves each row's `selected` from focus and preview. Returns
  the window bounds too, so the click hitboxes come from the same computation
  that produced the rows rather than a parallel one.
- `tasks_tree(rows, focused) -> ViewNode` — geometry-free. One `Line` per row
  inside a `List`, or the muted empty-state line.
- `render_tasks_panel` paints the block, reserves the focused action footer, and
  hands the tree to `ui::plugin_pane::render_tree`.

The old span-building row builder is **kept as a `#[cfg(test)]` oracle** and a
differential test asserts the tree renders cell-for-cell identically, exactly as
`legacy_lines` does for the info panel. That is what makes "the pane's rendering
is unchanged" a check rather than a claim.

## 5. The divergences this port leaves open, and what would close them

Both are geometry, both are pinned by a test in `tests/bundled_tasks_panel.rs`,
and neither is closed here because each wants to be designed from more than one
pane's needs — which is the lesson §4 records about the gauge.

| Divergence | Native | Plugin's copy | Cheapest closure |
|---|---|---|---|
| a title wider than the column | fitted with `…`, room reserved for the `⇄` | clipped at the pane edge by the renderer, `⇄` clipped with it | a `line` that clips with an ellipsis, and a flush-right run — the `gauge` suffix already right-aligns, so the mechanism exists inside the renderer |
| more tasks than rows | windows around the selection | draws from the first row and is clipped at the bottom | a list node carrying a selected index, windowed by the kernel from the height it has |

The second is the one the session-list port will have to close, since a session
list that cannot scroll to its selection is not a session list. Recording it from
here means that port starts with the requirement rather than discovering it.

## 6. What did *not* need widening, recorded because it is the result

- **No new style token.** The pane draws `text_primary`, `accent` and
  `text_muted`, all reachable from ADR-26's vocabulary.
- **No new container.** A `list` of `line`s is the whole pane.
- **No formatter.** PHASE4 §7 predicted every plugin would reimplement
  `format_bytes`; this pane formats nothing at all, so the case for a
  `thurbox.format.*` table is still made by exactly one pane.
- **No architecture edge.** Every new type sits in the module that already owns
  its kind of data, so `tests/architecture_rules.rs`, `CLAUDE.md`'s architecture
  section and `docs/CONSTITUTION.md` are untouched.
