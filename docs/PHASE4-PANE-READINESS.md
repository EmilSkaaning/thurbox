# Phase 4 pane readiness — what a bundled pane still cannot do

Phase 4 of the v2 migration ports thurbox's panes to plugins, easiest first:
info panel, tasks, automations, file viewer, global search, code review, session
list. The honest question each port has to answer is not "did it render" but
**did the plugin API suffice, or did it have to be widened** — because that is
the only real evidence about whether a third party can build a pane like ours.

This document is that audit, run against the **info panel** (order 1) before
porting it, with every claim traced to the code that makes it true. It is a
worklist, not a design: each row is a gap someone has to close, with the
cheapest honest closure named.

Status of the audit: **all five gaps closed** (inline lines, commit `6e0c7cc`;
style tokens and gauges, ADR-26; kernel state, ADR-27; the layout and the
keyboard, ADR-28). §8 records what the **second** port — the tasks pane, ADR-29 —
needed on top of them, and §9 the **third** — the file viewer, ADR-30, which
closed §8's scrolling row. §10 records the first surface on the phase's list that
was **not ported at all**: global search is not a pane, and no widening of the
pane API would have made it one. §11 records the **fourth** port — the code
review's diff stream, ADR-31 — the first ported only in *part*, and the first to
reach the host's execution and node bounds. Those four sections are the only part
of this document still a worklist.

The info panel has now been ported twice — first to the view tree in every
build (ADR-26), then reproduced as the **bundled `info-panel` plugin** (ADR-27) —
so the rows below are no longer predictions. Where a prediction was wrong, it
says so.

The plugin is a reproduction, not a replacement: the native pane is still what
thurbox draws, and `tests/bundled_info_panel.rs` asserts the plugin's view tree
*equals* the native pane's, so the two cannot drift while both exist. Two costs
the port did not pay off are recorded in §7 rather than closed.

## 1. Closed: a line of differently-styled runs

`ui.row` splits its area into equal shares
(`Constraint::Ratio(1, n)` in `ui/plugin_pane.rs`) and a single `ui.text`
carries exactly one style. Between them they could not draw
a muted `Name:` label followed by an unmuted `demo` — which is what
`append_session_section` is eight of, and what every list row in thurbox is.

Closed by `ViewNode::Line`: runs packed on one row at their own display width,
holding only nodes whose width follows from their content. See
`openspec/specs/plugin-host/view-tree/spec.md`.

## 2. Closed: no host binding read kernel state

`plugin::capabilities::build_module_table` granted exactly four things: `name`,
`log`, the `state*` trio over the plugin's own key/value namespace, and the `ui`
constructors. **There was no binding through which a plugin could read a session,
a task, an automation, or anything else the kernel owns** — so a pane that
renders kernel data could not be written at all, not badly, not at all. The
session-list spike hit this too and had to *model* a `sessions()` binding to take
its measurements.

**Closed by a published snapshot** (ADR-27), on exactly the shape this row
proposed: `session::pane_context` holds pure data plus a process-wide
`RwLock<Option<PaneContext>>`, `app` publishes it on the tick, and `plugin`
reads it when a plugin calls a reader. No new architecture edge, no plugin call
on the UI thread. Both properties the row said had to be designed were:

- **The read is capability-gated — three of them.** `sessions`, `metrics` and
  `automations`, each gating one reader. The row proposed one (`sessions`);
  porting the pane showed one was wrong, because the pane also draws host CPU and
  scheduled automations, and a plugin that wants a session name must not have to
  demand host telemetry to get it. The capability list is the install prompt.
- **Publishing is not per-tick work.** Two gates: nothing is built unless a
  running plugin holds a state capability, and nothing is written unless the
  value changed. Asserted on the `pane_context_builds` / `pane_context_publishes`
  counters, not in prose. The row proposed an input signature on the
  `session_order_signature` pattern; ADR-27 rejected it with a reason — over
  these inputs a signature must touch every field the snapshot touches, so it
  saves allocations rather than traversal, and it adds a second description of
  the snapshot's dependencies that can drift from the snapshot.

**What the snapshot does and does not carry** turned out to be the whole design.
A VM loads no `os` and no path library, so the kernel resolves what a plugin
*cannot compute*: a countdown (never an absolute instant), a directory's display
name (never a path), a parent session's name (never only its id), and each
status's glyph **and style token**. Everything else is a number, and the plugin
composes every string it draws. Publishing `"8.0/16.0 GB"` would have made the
port trivial and worthless — the plugin would have been arranging strings the
kernel composed, which proves nothing about a third-party pane.

## 3. Closed: five style tokens cannot address the palette the pane uses

`StyleToken` offers `accent`, `muted`, `danger`, `success`, `warning`, mapped in
`ui::plugin_pane::token_color` onto `accent`, `text_muted`, `danger`,
`status_idle`, `status_working`. The info panel draws from a wider set of
**distinct** palette fields: `text_secondary`, `role_name` (the agent name),
`branch_name` (repos), `status_blocked`, `status_done`, `status_unreachable`.

The consequence is specific, not aesthetic: a plugin **cannot draw a session's
status dot in that status's colour.** `Done` and `Unreachable` have no token at
all, and `danger` is a separate palette field from `status_blocked` — equal in
the default theme, not equal by definition, and a custom theme may set either
alone.

**Closed by eleven more tokens** (ADR-26), each named for and resolving 1:1 onto
the `ThemePalette` field it addresses: `secondary`, `role`, `branch`, `added`,
`border`, and one per session status. The audit proposed closing only the status
roles and calling the rest "weaker cases"; porting the pane showed that was the
wrong line to draw. A weaker case still has to be *drawn*, and a pane that
approximated `role_name` with `accent` would not have been byte-identical — so
the criterion, not taste, decided it. The direction of the constraint held: no
node may name a colour.

Two overlaps are deliberate. `warning`/`status_working` and
`success`/`status_idle` resolve alike today, because the first of each pair is a
token a plugin picks for *its own* meaning and the second is the token for *the
kernel's* session status. Collapsing them would leave a pane drawing status
indicators picking `warning` for working and `status_blocked` for blocked.

## 4. Closed: a gauge needs a width the tree does not carry

`info_panel::render_gauge_lines` right-aligns its suffix by computing
`padding = width - label - right` and sizes its bar to `width - 2`. Both need
the **resolved pane width**, and a plugin has no access to one: the view tree
carries no geometry, and no binding reports a pane's rect.

This blocks every gauge in the pane — session CPU, system CPU and RAM, the agent
context window, and each account-usage window — which is the majority of the
panel's visual weight.

There are two ways out and they are not equivalent. A **`gauge` node** (label,
percent, optional suffix) keeps geometry inside the kernel, is trivially
theme-aware, and is the shape ADR-V14's widget set already assumes; it also
bounds what a plugin can ask for. Reporting the **resolved rect back to the
plugin** is the general answer and the worse one: it makes rendering
width-dependent, which means a resize has to re-enter the VM before the frame
that needs it, and a plugin that mis-measures produces a broken pane rather than
a refused node. Prefer the node.

**Closed by the node** (ADR-26). One thing the audit got wrong and the port
found: a gauge is *not* reliably two rows. When `label + suffix` exceeds the
width the padding is zero and v1's header **wrapped**, pushing the bar down — so
the node's height is `header rows + 1`, not `2`. The first implementation clipped
that header, and the differential test against the retained pre-port renderer
caught it. That is the argument for writing the oracle rather than eyeballing the
port.

The pane now needs no width at all: `ui::info_panel::info_tree` takes no area,
which `the_tree_carries_no_geometry` asserts. That, not the gauge itself, is the
property a plugin would need.

## 5. Closed: the layout seats N panes, and so does the keyboard

Unchanged by the info-panel port: the pane is read-only and takes no keys, so it
neither needed nor exercised this.

**Closed.** The layout no longer hosts a single pane. `ui::layout` divides the
screen with a workspace tree (ADR-24), the right column holds one region per
*visible* plugin pane (`PanelAreas::plugin_panes`), and
`App::render_plugin_panes` draws each of them. Two bundled panes can be on
screen at once, which is what `two_plugin_panes_both_reach_the_screen` in
`src/app/acceptance.rs` asserts.

**Closed by the keyboard half** (ADR-28). What was open was not cosmetic:
`App::toggle_plugin_pane` mutated `plugin_panes.first_mut()`, and with `hello`
and `info-panel` both declaring a pane that meant **the pane §2 shipped could not
be put on screen by any key** — only by `thurbox-cli command run
info-panel.info.show` or by editing the stored choice. So the answer to "did the
port work" was, for a keyboard user, no.

`Action::TogglePluginPane` now toggles directly with one declared pane and opens
`Modal::PluginPanes` with several (none: it does nothing). The alternative this
row proposed — generated per-pane commands as *chords* (ADR-V21) — was rejected
with a reason: `session::Action` is a fixed enum whose order indexes the F1
editor's rows, so generating variants per discovered pane would make the
keybinding namespace depend on which plugins are installed. ADR-V21's generated
commands remain the right answer for the *name-addressed* case and already exist
headlessly; the picker is the answer for the keyboard.

**And the related measurement is answered.** `render_all_panes_collected` no
longer renders a pane the kernel is hiding: `session::pane_visibility` publishes
the hidden set on the tick (change-gated, counted by
`pane_visibility_publishes`) and the host consults it before entering a VM. The
cost this removes is exactly the one the motion work refused to pay for a hidden
pane, and it was being paid by every default install with two bundled panes. The
skip is asserted on `PluginHost::render_calls` rather than on the returned
results, because a pane filtered before the call and one rendered and discarded
produce the identical list — which is how the discarding version survived a
review at all.

One cost accepted in exchange: a hidden pane's tree goes stale, so unhiding shows
its last tree (or `loading`) for up to one worker cycle. That is §7's staleness,
now paid once on a keystroke instead of every second forever.

## 6. What the audit implies about ordering

The migration plan calls the info panel the easy one because it is read-only and
has no input. That is true of its *input* surface and misleading about its
*data* surface: it is the pane with the most kernel state and the most geometry
per line in thurbox. Of the five gaps above, the tasks pane needed §2 and §5 and
neither §3 nor §4 — so it would have been the cheaper first port, and §2 is now
closed for it too.

So the cheapest first bundled pane was not the first row of the table. That is a
finding about the plan, not a proposal to change it — but the next person should
weigh gap §4 before assuming "read-only" means "easy".

Two further findings the port produced, for whoever ports the next pane.

**A missing gap: nothing in the catalogue wrapped.** The audit listed geometry
(§4) but not *reflow*. `ViewNode::Line` clips at one row by construction, while
every row of the info panel was drawn by a `Paragraph` with `Wrap { trim: false }`
— and it must be, since `Activity` and `Signal` carry agent-supplied text of
unbounded length. Closed by `ViewNode::Paragraph` (ADR-26). The lesson
generalises: the audit was written by reading what the *catalogue* offers, and
the gap was in what the *pane* relies on. Reading the pane's ratatui calls, not
the node list, is what finds these.

**The pane is now a plugin.** The rendering claim was narrow on purpose — the
catalogue could express this pane's *rendering*, not that a third party could
have written it — and closing §2 is what made the wider claim available. The
bundled `info-panel` plugin produces the identical view tree from three declared
capabilities and nothing else.

## 7. What the plugin port cost, and did not pay off

Two things the bundled pane made measurable rather than arguable. Neither is
closed, and neither should be closed by the next port either without deciding it
deliberately.

**The plugin pane is up to a second behind.** The render worker polls on a
~1 s cycle (`PLUGIN_RENDER_SLICE` × `PLUGIN_RENDER_SLICES` in `src/main.rs`), so
a live gauge in the plugin's copy of the pane lags the native pane's. Nothing
about the snapshot causes this — it is republished on the tick — and nothing about
the pane hides it: side by side, the plugin's CPU bar visibly trails.
`docs/SPIKE-SESSION-LIST.md` already fixed **event-driven render** as a condition
of the session-list port; this is the second pane to want it, which makes it a
scheduling decision rather than a session-list detail.

**Every plugin will reimplement `format_bytes`.** The snapshot publishes raw
numbers on purpose, so the bundled pane carries eight formatters in Luau
(`humanBytes`, `formatBytes`, `formatBytesPair`, `formatCost`, `formatDuration`,
`formatTokens`, `formatCountdownSecs`, `formatDueIn`) — about 80 lines that any
pane showing a byte count or a duration will write again. A `thurbox.format.*`
helper table would fix it. It is deliberately not added here: it should be
designed from two or three panes' needs rather than one, and adding it in the same
change would have destroyed the evidence that a plugin can own its presentation
at all. Whoever ports the second pane is the right person to decide.

A third thing worth recording because it did *not* happen: **the view tree needed
no widening.** An independent consumer needed `list`, `paragraph`, `divider`,
`gauge` and `text` with eight tokens, every one of which ADR-26 had already added
for the native port. That is the confirmation ADR-26 could not give itself.

## 8. The second port: the tasks pane (ADR-29)

§6 predicted that the tasks pane needed §2 and §5 and neither §3 nor §4. Both
halves of that were wrong in an interesting way, and this section is the record.

**What sufficed.** Everything the pane *renders* was already expressible: a `list`
of `line`s, three colour roles (`accent`, `muted`, and the theme's primary
foreground as a token-less run) — no new node kind, no new style token, and no
formatter at all. So §7's prediction that every pane would reimplement
`format_bytes` is still made by exactly one pane, and a `thurbox.format.*` table
stays undesigned on purpose.

**What had to be widened: two emphasis flags** (`dim`, `underline`), ADR-29. This
is §3 again, and the prediction that the tasks pane would not need it was wrong
for a reason worth stating: §3 was written about *colour*, and a selectable row
needs three appearances that are not three colours. `Theme::selected_item()` is
accent + bold and was expressible; a row a running search filtered out is muted +
**DIM**, and a matched character is accent + bold + **UNDERLINED**, and neither
attribute existed. The consequence was not aesthetic: **no plugin could draw a
list with a search in it**, which is every remaining pane in the phase. Reading
the pane's ratatui calls is what found it — reading the node catalogue would not
have, exactly as §6 says.

**What is still open: geometry, in two named pieces.** This is §4 again, and here
the prediction was right that the pane does not need a *gauge* while being wrong
that it needs no geometry. Three of the pane's decisions depend on its resolved
rect, and the port left all three in the kernel
(`ui::tasks_panel::visible_rows`), which means the plugin's copy of the pane
differs in exactly these ways. Both are pinned by their own test in
`tests/bundled_tasks_panel.rs`, so neither can be forgotten or quietly absorbed:

| Open gap | Native pane | The plugin's copy | Cheapest closure |
|---|---|---|---|
| a title wider than the column | fitted with `…`, with the `⇄` marker's width reserved | draws the whole title; the renderer clips it, and a linked row can lose its marker | a `line` that clips with an ellipsis, plus a flush-right run — the renderer already right-aligns a gauge's suffix, so what is missing is a node that asks for it |
| more rows than the pane has lines | windows around the selection | draws from the first row and is clipped at the bottom, so a selection below the fold is invisible | a list node carrying a selected index, windowed by the kernel from the height it has — the `gauge` shape, applied to height |

The second is a **precondition of the session-list port**, not a nicety: a session
list that cannot scroll to its selection is not a session list. Recording it here
means that port starts with the requirement rather than discovering it.

**A rejection worth carrying forward.** The tempting fix for both rows is to
publish the rows *already* fitted and windowed, which would make the tree equality
total. It was rejected twice over: the publisher has no width (the snapshot is
built on the tick, a pane's rect exists only during a frame, and the native pane is
hidden by default anyway), and the plugin's pane is a *different rect in the same
layout* — rows fitted to another pane's width are wrong at their own. A pane that
renders its own rows plainly is better evidence than one that renders someone
else's geometry.

**And the state channel scaled.** §2's closure was a snapshot with three sections;
this port added a fourth and needed no new mechanism, no new architecture edge, and
no change to the demand/change gates. The one thing it did establish is where the
line falls: the kernel publishes a status's *name* here, where it publishes a
session status's glyph and token in `StatusSnapshot` — because that mapping is
shared by two native panes and this one is not. "Publish the rendering only when
two panes must agree about it" is the rule the next port should apply.

## 9. The third port: the file viewer's tree (ADR-30)

§8's table left two open geometry rows and called the second one — a list that
cannot scroll to its selection — a *precondition of the session-list port*. The
file viewer is the pane that could not defer it: its whole interaction is moving a
cursor through a tree taller than its column, so a copy drawing from row 0 would
not be a reproduction. **That row is now closed**, and closed for every remaining
pane rather than only this one.

**What closed it.** `ViewNode::List` carries an optional selected child, and when
a list declares one the *renderer* chooses the visible slice — through
`ui::file_viewer::visible_window`, the same helper the native panes already
shared. That is the `gauge` trade applied to height (ADR-26), and it buys
something the tasks port could not claim: this pane's equality is not only tree
equality but **frame** equality at a size where the pane scrolls
(`the_plugin_paints_the_native_frame_when_the_pane_scrolls`). The other way out —
report the resolved rect into the plugin — was rejected for the third time, for
ADR-26's reasons.

The consequence for `ui::file_viewer` is worth noting: `file_tree` takes no
geometry **at all**, not even a window. It is the first pane tree of which that is
fully true (`tasks_tree` still receives rows someone fitted and windowed).

**The second widening was not a colour and not an emphasis.** The file viewer draws
its cursor's row with a *background* (`selection_bg`/`selection_fg`), and nothing in
the catalog could name one. `TextStyle::selected` is a **role**: the plugin says
"this run is on the selected row" and the theme owns both colours. It *replaces*
the token's colour rather than layering, unlike the three emphases — which is
also what the native pane does.

The tempting alternative, and the one worth recording as rejected: let the *list's*
selected index drive the appearance, since the kernel already knows which row it
is. It cannot, and the reason is a fact about thurbox rather than about the
catalog — **thurbox's two list panes disagree about what a selected row looks
like.** The tasks pane draws it `accent` + bold (`Theme::selected_item()`), the
file viewer draws it in the selection pair. An appearance inferred from the anchor
would have made one of the two unreproducible. So the anchor is the list's and the
appearance is the run's.

**The capability came out narrower than the brief predicted, not wider.** The port
was specified as needing a `files` capability that could "list a directory and read
a file's lines", with the correct observation that this would be the widest power
granted to a plugin so far. It turned out the pane needs **no filesystem access at
all**, and the finding generalises: of the five facts a row draws, only its name
comes from disk. Depth and expansion state are the *user's* navigation, `matched`
is a search the kernel runs, and the cursor is the keyboard's. A plugin holding
`read_dir` could therefore draw *a* file tree but not *this pane* — its tree would
have an expansion state nobody chose, no cursor and no search, and the equality
test that is a Phase 4 port's deliverable could not have been written.

So `Capability::Files` reads a published section and nothing else. It is named
`Files` rather than `Fs` deliberately: `tests/teardown_gate.rs` reserves
`Capability::Fs` for v1's "place a file in an agent's own config dir" power, and
that row stays blocked — adding a filesystem binding here would have advanced a
teardown verdict as a side effect of drawing a tree. What the section carries and
what it refuses is tabulated in the change's `design.md` §1; the short form is
basenames, no paths, no unexpanded directories, no dotfiles, no query, and no I/O.

**One thing the kernel does publish that is not a fact about files:**
`nerdFont`. The markers are chosen from two glyph sets by a display setting, and
§8's rule (publish a rendering only when two panes must agree) says the *fact*
crosses and the glyph does not — `src/ui/file_viewer.rs` is the only reader of
`nerd_font_enabled` outside the theme. It rides on the file section because that is
its only consumer; a second consumer should lift it to its own section rather than
a copy appearing.

**What is out of scope, and said so.** The search **sub-mode's bar** — the
three-row bordered `Search (2/5)` block below the tree, with its slash prefix, the
query scrolled to its end, and a block cursor — is not reproduced, and could not
be. Three host features are missing, and naming them is the point of this
paragraph:

| Missing | Needed for |
|---|---|
| a bordered/framed container node | the bar's own block inside the pane |
| a cursor appearance (a reversed cell) | the caret in the query |
| a bottom-anchored fixed-height region | the `Min(0)` + `Length(3)` split the pane makes of its own area |

and the match counter would need the query *text*, which the capability
deliberately does not publish. The search's **effect on the tree** is ported and is
part of the equality test, so the record distinguishes "cannot be drawn" from "was
not attempted". A port that had quietly dropped the row emphasis too would have
looked cleaner and proved less.

**Still open, pinned by tests:**

| Open gap | Native pane | The plugin's copy | Cheapest closure |
|---|---|---|---|
| the search bar | a bordered block carrying a query, a caret and a match counter | nothing; the rows still show the search's verdict | the three nodes above |
| the scrollbar | a reserved rightmost column with a draggable thumb | nothing | a `scrollbar` field on the list node — the renderer already resolves the window, but the native pane reserves its track *outside* the tree, so moving it in is Phase 6 work. **Closed in §16**, which is that work: the reservation moved into the renderer and the native pane's rows landed in the rect they were already painted into |
| the tree is the pane's, lazily | filled on toggle, session change, or a search reveal | `No folders` until the native viewer has been opened once | rebuild the state on session change rather than on first draw |

The third is a limitation of the *section*, not of the catalog, and it is a
deliberate trade: filling the tree from the publisher would mean the presence of a
plugin decided when thurbox reads directories.

**§8's first row is still open.** Nothing here needed an ellipsizing clip or a
flush-right run — the file viewer does not fit its labels, it lets the renderer
clip them — so that row is unchanged and still waits for a pane that does.

**And what needed nothing.** No new style token (`accent`, `muted` and the
token-less primary foreground drew every row). No formatter, so §7's
`thurbox.format.*` case is *still* made by exactly one pane after three ports —
which is now strong evidence that it should stay undesigned. No new architecture
edge, and no change to the demand or change gates: the snapshot took a fifth
section the way it took its fourth.

## 10. Global search: the first surface that is not a pane

The three ports above answered "did the API suffice" with a widening. Global
search answers it differently: **no widening of the pane API would have
sufficed, because the surface is not a pane.** It is recorded here rather than
ported, and no bundled plugin was shipped. The blockers below are re-derived from
the source by `tests/global_search_pane_gap.rs`, so closing one fails that test
and names it — a verdict in prose expires without telling anyone.

**What the surface does**, from `src/app/search.rs`, where only the first item is
a rectangle:

1. draws a strip — query, per-scope counts, a grouped result list, key hints;
2. **computes** the search: fuzzy metadata over sessions, tasks and automations,
   a substring scan of the active session's file tree, and a debounced scan of
   **every session's live vt100 screen** (`App::session_content_match`);
3. **restyles rows in three panes it does not own** — matched characters accent +
   bold + underlined, unmatched rows muted + dim (`ui::highlight`, applied inside
   `project_list`, `tasks_panel` and `automations_panel` from the query the view
   hands each of them);
4. **moves those panes' cursors** as a live preview, and forces a hidden panel
   visible to do it;
5. **takes focus** on `Enter`, or restores focus, three selections and two panel
   toggles from `SearchSnapshot` on `Esc`.

Items 3–5 are the definition of a mode: a surface that owns the whole interface's
input and appearance while it is up. A plugin pane is a rect plus its own keys,
and each of the four walls it hits is a decision with a reason attached, not an
oversight.

**The precise claim, because the loose one would be wrong.** The search's
*verdict* already crosses the boundary: `TaskSnapshot` publishes `dimmed` and
`match_positions`, `FileNodeSnapshot` publishes `matched`, and that is how the
bundled tasks and file-viewer panes reproduce v1's dim-and-underline appearance
exactly. A plugin pane is therefore a pane the search *affects*. What has no path
is the other direction — being the surface that *produces* the effect — and every
row below is one form of that asymmetry.

**Structural — no node closes these:**

| Blocked | Where the host stands | What it would take |
|---|---|---|
| a full-width band above the footer | `PaneSlot` is a closed set whose only member is `Right`; a plugin pane is seated only by `LayoutParams::right_regions`, while the strip's band is `RegionId::GlobalSearch` | a band slot **and** a declared height, which is the geometry the model has refused a plugin three times (ADR-26, ADR-29, ADR-30) |
| the query and the results | no capability publishes either | either the plugin reads every session's screen — the widest read in the application, 500 lines per session of raw agent output — or the kernel does the search and publishes the strip's *rendering*, which §8's rule forbids |
| **producing** the restyling of rows in other panes | the verdict already crosses *outward* (a published task row carries `dimmed` + `match_positions`, a file row `matched`), which is how the bundled tasks and file-viewer panes reproduce it — but each pane applies it to *its own* rows, and nothing carries a query the other way | a channel by which plugin state changes a *native* pane's appearance — so a plugin whose own pane is hidden could restyle the visible ones, which is the reach `pane_visibility` exists to bound |
| move a cursor, take focus, restore a snapshot | no binding writes **view** state. A plugin may now change *records* it was granted (ADR-35: a task's status, an automation's enabled flag), and nothing it holds moves a cursor, takes focus, shows a panel or switches the active session | a write channel over view state — i.e. any installed plugin may move the user's cursor and take focus |

**Vocabulary — cheap, and left open on purpose:**

| Blocked | Where the host stands |
|---|---|
| the strip's bordered block, titled ` Search ` | no frame node, and a pane's frame is the host's (`focus_block` in `App::render_plugin_panes`); §9 recorded the same gap for the file viewer's search bar, so this is its **second** consumer |
| a hint row pinned to the last line under a `Min(0)` list | `Column` stacks children at their natural height from the top; §9's "bottom-anchored fixed-height region" again |
| the search accent (`Theme::search_bar`) | `StyleToken` names no such role, and a plugin may name no colour |
| the italic snippet line under a content match | `TextStyle` carries bold, dim, underline and the selection role |

One correction that ADR-35 forced, recorded because a gate caught it: the row above
was probed by asking whether *any* write-shaped binding existed, and
`tasks-write`/`automations-write` added `setTaskStatus`, so the probe reported the
row closed. It is not — changing a record is not moving a cursor — and
`tests/global_search_pane_gap.rs` now distinguishes a view write from a record
write, with `a_record_write_is_not_the_write_the_strip_needs` pinning the reason.
This is the second time a probe has had to be tightened rather than a verdict
flipped (§11 records the first, a node named `Fill`), which is the argument for the
gates existing at all.

**What a plugin *can* build here today**, said so the record is not read as "the
API is empty": with `input` plus the state capabilities, a plugin can collect its
own query and render its own filtered list, in its own pane, over the published
sections — task titles, automation labels, the open file tree's basenames, and the
active session's name, agent and repo. What it cannot do is search the *other*
sessions, scan any terminal, restyle a native pane, or go to what was chosen. That
is a fuzzy picker, and a useful one; it is not global search.

They are not closed here because closing them would put a fourth emphasis and a
frame node in the catalogue for a pane that is not being shipped. A vocabulary
gap is worth closing when a pane needs it to *ship*; none of these does.

**The port that was available, and why it was refused.** Add a `search`
capability, publish `{query, results, selected}`, refactor the strip into a
`search_tree`, and ship a bundled pane that reproduces it. Tree equality was
reachable. It was refused because the pane would sit in the right column beside
the real strip — searching nothing, highlighting nothing, previewing nothing and
jumping nowhere — while the kernel kept doing all five behaviours and handed it
the pixels. It could not even own the query: a pane declaring `input` collects
keystrokes while *it* is focused, but nothing carries a query it collected into
`GlobalSearchState`, so what it typed would search nothing. Phase 4's own
rule is that *a gap worked around by a shortcut a third party could not take must
be recorded as still open*; here the shortcut is the feature. The same objection
that stopped ADR-27 from publishing `"8.0/16.0 GB"` stops this: a result's
`label` and `snippet` are the strip's output — ordered, capped at
`MAX_PER_GROUP`, truncated to 120 chars, for the strip — not kernel state a pane
interprets.

**The useful half of the finding: the shape is a provider, not a pane.** Global
search's *surface* is kernel-owned by nature — docked chrome, owning input,
editing other panes — and none of that is work a third party wants. What a third
party plausibly wants is to contribute a **scope**: search my notes, my open PRs,
my shell history. That asks for three things, each *narrower* than its pane-model
equivalent: a hook called with the query returning results as data (instead of
reading every session's screen); a result carrying an opaque target token the
kernel resolves (instead of a write channel into focus); and nothing at all about
the strip (instead of a band slot, a frame node, a token and an emphasis). It is
closer to the command registry — host-invoked, manifest-declared — than to a
pane. It is deliberately **not designed here**: two of the remaining surfaces in
this phase are also not plain panes, and a non-pane extension point designed from
one consumer is the mistake §7 warned about for `thurbox.format.*`.

**What this does not claim.** Not that the strip's *rendering* is inexpressible —
the four vocabulary rows are the whole distance, and the record says so. Not that
a tree-equality oracle was attempted and failed: none was written, because
refactoring `render_global_search` into a `search_tree` is the first step of the
port that is not happening. The vocabulary rows come from reading the renderer's
ratatui calls, which §6 named as the method that finds what reading the node
catalogue misses. And no ADR was added, deliberately: nothing about the host
changed, and the rejected alternatives live in the change's `design.md`.

**What it means for the two surfaces left.** The phase's remaining order is code
review and the session list, and neither is a plain pane either: the code-review
view owns the central pane, a compose sub-mode and a column in another slot,
while selecting a row in the session list *is* switching the application's active
session (a write, blocked by the same wall as row four above). So the honest
reading of §6's "ordering" finding is stronger than it was: the phase's list is
not sorted by difficulty but by *kind*, and the panes are done. What is left needs
the non-pane extension point, which is the next thing to design rather than the
next thing to port.

## 11. The fourth port: the code review's diff stream (ADR-31)

§10 read the phase's remaining list as "not sorted by difficulty but by *kind*",
and said the code-review view is not a plain pane either. That reading holds and
this section is what came of taking it seriously: the pane is ported **in part**,
the part is reproduced completely, and the remainder is a list rather than a gap.

It is also the first port to answer a question the three before it could not ask.
Every earlier pane draws one row per record with two or three runs on it. A diff
row is a gutter, one run **per syntax token**, and a background that has to reach
the pane's right edge — thousands of times over. So this is where the view tree's
bounds get measured against a real pane rather than against a fixture.

### What was ported, and what was not

**In scope:** the unified stream's *lines*. For each one, the
`{old} {new} {sign}` line-number gutter in the muted role, the body tokenised and
drawn one styled run per token, the insertion/deletion row tint carried to the
pane's right edge, and the cursor's row in the theme's selection pair.

**Out of scope, itemised** — the point of this table is that the remaining surface
is enumerable:

| Not ported | Why |
|---|---|
| the side-by-side layout | `paired_body_width` divides the pane's resolved width in two; no node carries a width |
| the wrap toggle | `unified_diff_line_wrapped` chunks a body by the *available* width, and its chunk boundaries are its own arithmetic over a number the plugin is never told |
| horizontal scroll | both window bounds are geometry |
| file headers and hunk headers | expressible in shape (the new fill node is what a header's trailing rule wants), but the counts are drawn in `diff_added`/`diff_removed` — separate palette fields from the `added` token's `tool_allowed` — and the hunk header is `truncate`d with an ellipsis, which is §8's still-open clipping row |
| comments, classification badges, the review summary | a second published shape and a second interaction; the stream is what is being measured |
| reviewed marks and folding | they belong to the headers, above |
| the find sub-mode | its bar needs §9's three missing features (a frame node, a cursor appearance, a bottom-anchored region), and its in-row match highlight *replaces* the syntax colouring, which is a third styling mode |
| the target picker, the footer, the compose box | chrome and sub-modes, each owning keys a plugin pane does not receive |
| the scrollbar | chrome outside the rows. It was the file viewer's position too until §16 moved that pane's track into its list node; this pane's stays outside, because a review's track is drawn beside a *centre*-pane surface a plugin cannot be seated in |
| the central-pane seat | `PaneSlot` seats a plugin pane only on the right; the native review owns the centre *and* a column in the right slot |

`the_out_of_scope_surface_is_absent_rather_than_approximated` asserts the plugin's
pane contains none of it — no `@@`, no rule, no chevron, no `✓`, no badge, no
`Search`. A port that had drawn a plausible-looking file header would have looked
more complete and proved less.

### What had to be widened: three things, all of them roles or residue

**A tint** (`TextStyle::tint`, two members) — because the row background is the
*only* thing distinguishing an insertion from a deletion in the body. The gutter's
sign is one character and the body's colours belong to the syntax highlighter, so a
"port" without the tint would not be a port of add/remove colouring at all. Like
§9's `selected` it is a role the theme resolves; unlike it, it leaves the
foreground to the token, and selection beats it.

**A fill** (`ViewNode::Fill`) — because a background that stops where the text
stops is not the row. This is the `gauge` trade a fourth time, applied to a line's
leftover columns, and it is **half of §8's first open row**: put a fill *before* a
run and the run is flush right. The ellipsizing clip that row also asks for is
still open, because nothing in this scope truncates.

**One style token** (`accent_bright`) — the colour `ui::syntax` gives a capitalised
type name, and the only one of the six it uses that no token could name.

And a fourth thing that is not a node at all: **the sandbox loaded no way to walk a
string by character.** `StdLib::UTF8` was absent, so a plugin lexing a line
containing one multi-byte character drifted for the rest of it. It is pure
computation — no file, no process, no clock — so it is admissible for the reason
`math` is, and it is *necessary* rather than convenient for any pane that styles
the inside of a line. This is §6's lesson again: reading the pane's calls found the
node gaps, and only running the plugin found this one.

### What did **not** have to be widened, which is the stronger half

**The kernel publishes no colouring.** The `review` section carries a line's raw
text and its file's path; the bundled plugin carries the lexer, in Luau. Publishing
`{text, token}` runs would have been smaller, shorter and faster, and it was
rejected on §8's rule — publish a rendering only when two panes must agree about it
— which bites hardest here, because syntax highlighting is the most obviously
presentational thing in the pane and `src/ui/code_review.rs` is `ui::syntax`'s only
reader. The cost is real: the two lexers must agree token for token, and nothing
but the equality test makes them.

**No filesystem and no git.** `Capability::Review` reads the diff the *user*
opened. `Capability::Fs` stays undeclared (the teardown gate reserves it), and the
vocabulary defines no `git` either.

**No formatter.** After four ports §7's `thurbox.format.*` case is still made by
exactly one pane.

### The oracle is different here, and the difference is the finding

The three earlier ports refactored the native pane to *draw* its view tree, so tree
equality was frame equality by construction. This one does not, because
`unified_diff_line` cannot be a geometry-free tree: it windows the body to
`[h_scroll, h_scroll + avail)`, slices that window by **character count** against a
resolved width, and the wrap mode reflows one logical row onto several by the same
arithmetic.

So the chain has two links: `tests/bundled_code_review.rs` asserts the plugin's
tree equals `ui::code_review::diff_stream_tree`, and `ui::code_review`'s own tests
paint `diff_row_tree` against the **untouched** `unified_diff_line` and require the
frames to be identical. Without the second link the first would be two functions
written in the same change agreeing about a format neither is obliged to match —
which is exactly the shortcut Phase 4 asks a port not to take. The consequence to
state plainly: the reproduction is validated at the level of **one painted row**,
plus tree equality for the stream around it. It is not a claim that this pane's
rendering is now the tree's.

Two divergences fall out of that and are pinned by their own tests rather than
smoothed over:

| Divergence | Native | The tree |
|---|---|---|
| a tab in the body | the raw byte reaches the terminal | sanitized to four spaces on the way into a node |
| letter case outside ASCII | Rust's `char::is_uppercase` is Unicode-aware | the Luau lexer classifies case for ASCII only |
| a blank pad cell's foreground | unset (`Style::default()`) | the theme's primary foreground |

The third is why the frame comparison requires symbol, background and modifiers to
match everywhere and the foreground only where the cell is not blank: a space
carries no ink. Every cell that *does* carry ink must match exactly.

### The host's bounds, measured — and the documented one is not the tighter one

This is the most transferable thing the port produced.

A row of real code (`let total: Vec<Row> = rows.iter().map(|r| r.id).collect();`)
costs about **26 nodes** — a gutter, a fill, and one per token. `MAX_NODES` is
4096, so the node budget permits roughly **150 rows**: a diff of a few hundred
lines does not fit, and a refused tree means the pane shows an error rather than a
shorter diff.

But at that size the plugin is **not** refused for its node budget. It is refused
for its **execution** budget (`interrupt_budget`, 200 000 ticks), reached while the
Luau row loop is still running — the tree is never returned, let alone converted.
So the effective ceiling on a plugin diff pane is instructions, and the node budget
is the second wall behind it. `the_hosts_bounds_are_reached_by_an_ordinary_diff`
asserts both: that the node budget alone would allow under 300 rows, and that the
error at that size names the execution budget.

`MAX_REVIEW_ROWS` (60) is therefore a bound on the **section**, chosen so a
representative row leaves both budgets comfortable — not so a pathological one is
impossible. Nothing prevents a single dense 4096-character line from costing
hundreds of nodes on its own, which is the precise sense in which this is the first
pane the model cannot bound locally: *every other section bounds a row count
because a row costs a fixed handful of nodes.*

**Still open, and deliberately not designed:**

| Open gap | Where the host stands | Cheapest closure |
|---|---|---|
| the budget is spent on rows the kernel windows away | the plugin builds every published row; the kernel picks the visible slice afterwards | window *before* conversion — a lazy row source, or a declared row budget the plugin is told. Both are shapes the model has refused for width and height (ADR-26, ADR-29, ADR-30), and one consumer is too few to design a third |
| a per-row node budget | `MAX_NODES` is a whole-tree bound, so one dense row can refuse a whole pane | a per-child budget at conversion, refusing the row rather than the tree |
| §8's ellipsizing clip | still open; the fill closed only the flush-right half | a line that clips with an ellipsis |
| the four vocabulary rows §10 left open | unchanged (frame node, bottom-anchored row, search accent, italic) | this port needed none of them, and the find bar it would have drawn is out of scope |

One thing to note about that last row, because a gate caught it:
`tests/global_search_pane_gap.rs` listed a node named `Fill` as one of the shapes
that would close §10's *bottom-anchored region*, and this port added a node with
that name — so the probe reported the row closed. It is not: this fill is an
**inline** run whose width is the residue of a *line*, and a horizontal residue
anchors nothing vertically. The probe now asks the tree whether the fill it found
is inlineable rather than trusting its name, which is the correction the gate
existed to force.

### What it means for the last surface

The session list is what is left, and §10 already said selecting a row in it *is*
switching the application's active session — a write, which the kernel-state
channel does not do. Nothing here changes that. What this port adds to the record
is a second, quantitative reason to design the non-pane extension point before
porting it: the session list is small, but it is the pane most likely to be
**rebuilt often**, and this pane established that the binding constraint on a
plugin pane is not what it can express but how much work it may do per frame.

## 13. The fifth port: the session list, the pane ADR-V1 hinges on (ADR-33)

Numbered 13 because §12 and ADR-32 are reserved for the automations pane, whose
port is in flight on another branch; a hole is cheaper to read than two sections
with the same number.

This is the gate. ADR-V1 says everything but six things is a plugin **including
the session list**, and §11 closed by naming it as the last surface. It is the
densest pane thurbox has, the one redrawn most often, and the one whose ordering,
nesting and status rules the kernel owns.
`docs/SPIKE-SESSION-LIST.md` measured whether it could be a plugin at all and
answered *yes, on three conditions*. This section re-checks those three, because a
conditional verdict that nobody re-checks is a verdict that expires quietly.

### The headline: the drawing surface needed nothing new

No node kind, no style field, no style token, no capability. The whole pane is
four node kinds — `list`, `line`, `text`, `fill` — and it is pinned by
`the_host_surface_needed_no_new_node` rather than asserted here, because "the API
sufficed" is the claim ADR-V1 rests on and a claim in a document expires without
telling anyone.

What *did* grow is the **state** surface, by one reader: `thurbox.sessionList()`,
under the `sessions` grant that already existed for it. That is the shape every
port has had — a pane cannot draw records nobody publishes — and it is the
difference between "the vocabulary held" (which is the claim) and "nothing was
added" (which would not be true).

Two of those four earned their keep for the second time. `Line` (ADR-28) is what
the spike said had to exist before this pane could be attempted at all, and
`Fill` (ADR-31) — added six days earlier for a diff row's tint — is exactly what
a selection bar and a group header's trailing rule need. That is the first
evidence that `Fill` was a general node rather than one pane's escape hatch.

### The spike's three conditions, re-checked

| Condition | Verdict | Why |
|---|---|---|
| the catalogue needs a styled-span line node | **met** | `ViewNode::Line` (ADR-28), plus `Fill` for the residue a bar reaches across |
| selection stays kernel state | **met, and load-bearing** | the cursor is published per row, exactly as a task row's is; the plugin receives no keys and owns no cursor |
| render is event-driven, not a fixed poll | **not met** | measured below; recorded rather than worked around |

### What was ported, and what was not

**In scope:** every row the pane draws. The repo-group header and its trailing
rule, the status glyph in that status's colour — **animated** while the session is
working — the `└` nesting prefix and the `↳` cross-group child mark, the remote
and worktree marks, the name with the global search's matched characters
emphasised, the agent's activity text (or, when blocked, its notification), the
`selected > dimmed > role` precedence, and the cursor's row in the theme's
selection pair across the pane's whole width.

| Not ported | Why |
|---|---|
| the pane's border chrome | the `Sessions` block, the one-dot-per-session strip on its top border, and the `▲ N` / `▼ N` clipped-row indicators. A plugin pane's frame is the host's, and nothing in the catalogue describes a border overlay — §9 recorded the same gap for the file viewer's search bar |
| the empty state | `No sessions yet` / `Press Ctrl+N to create one` are drawn **centred**, and no node carries an alignment. A **new** vocabulary row, below |
| the pending-spawn placeholder row | a row for a session that does not exist yet, inserted at an index the kernel computes from the group layout, whose phase label is dropped by a width rule. A second published shape and a second geometry rule |
| scrolling by the native pane's rule | the plugin declares its cursor's row and the kernel windows the list (ADR-30); the native pane keeps ratatui's sticky offset, which its two-line items and click hitboxes are derived from. Both keep the cursor visible; they disagree about which rows sit beside it |
| keys and hitboxes | no `j`/`k`, no `Shift+J` reordering, no click. The cursor the pane draws is the kernel's. **Since ADR-34/35/36** a pane can declare rebindable keys, change the records those keys change, and receive a click resolved to one of its rows — so what is left for this pane is the reordering write and its own cursor, not the input model |

### The oracle is a refactor, and a two-link chain

The three list ports before this one refactored their native pane to draw its own
tree; the code-review port did not, because its painter is width-dependent in ways
no tree expresses (§11). This pane is in between, and the split is drawn at the
row: **the rows became trees, the list did not.**

- `ui::project_list::session_item_node` builds each row and header, and the native
  pane paints those nodes through the same inline walk `ui::plugin_pane` uses
  (`line_spans`). So a `Fill`'s residue is resolved by one implementation in both
  panes — two would be two panes disagreeing about where a selection bar ends.
- The ratatui `List` is left alone. Which rows are on screen, where a two-line
  item starts and which cell a click lands in are its answers, and the hitboxes
  are derived from the offset it actually used.

The chain therefore has two links, both asserted: the plugin's tree equals
`session_list_tree` (eleven content variants, `tests/bundled_session_list.rs`),
and each node paints what the **pre-port span builder** painted, cell for cell, at
two widths and for every spinner frame (`the_tree_paints_what_the_span_builder_painted`,
against the retained `legacy_session_line` oracle).

| Divergence | Native | The tree |
|---|---|---|
| a blank cell's foreground before the agent's text | `Span::raw`, which leaves the foreground unset | a token-less run, which resolves the theme's primary |

That is the only one, and it is pinned in both directions: `assert_same_ink`
grants it *only* on a cell with no glyph in it, and
`the_only_divergence_is_a_blank_cells_foreground` fails if it ever stops being
needed, so the latitude cannot outlive its reason.

### The spinner is declared motion, and it is inside the equality

ADR-V18 shipped with no bundled consumer. This pane is it: a working row's glyph
is a `cycle` of ten braille frames at 8 fps, keyed, pushed once — the frames are
the plugin's choice, the clock is the kernel's, and there is no call by which a
plugin asks for a frame.

The native pane declares the **same** node and resolves its frame through a
`FrameTable` filled from `App::spinner_frame()`, the clock it already ran on. Two
consequences worth stating:

- the equality covers the animation instead of exempting it — the alternative
  (native holds a text node, plugin holds a motion node, compare "up to the
  spinner") would have excused the one part of the pane that moves;
- `ui` still cannot reach a VM. The frame table is plain data, which is why
  `tests/architecture_rules.rs` is untouched by a pane that animates.

Deliberately **not** done: giving the native pane a real motion lease in
`App::motion`. Leases share a bounded aggregate rate, so putting thurbox's own
spinner in that budget would let an installed plugin's animation degrade it — a
regression introduced for tidiness.

### The render trigger: the spike's third condition does not hold

The plugin worker renders every pane, then waits out a **fixed 1 s interval** in
ten 100 ms slices serving key requests. Nothing tells it that kernel state moved.
So when the user presses `Ctrl+J`:

| | latency |
|---|---|
| the native pane's cursor moves | next frame — single-digit ms |
| the plugin's *copy* of that cursor moves | the worker's next cycle — **up to 1 s** |

The spike's bar 4 was 5 ms of added latency on a selection change, and it made the
verdict conditional on the render being event-driven. It is not, so the bar is
missed by ~200× — for a plugin's copy.

**Why that is a finding and not a defect in this pane.** The cursor a user drives
is kernel state (the spike's second condition), so the highlight the user actually
watches moves in the frame the key was handled. What trails is a reproduction of
it in a pane that is hidden by default. Had the plugin owned its own cursor — the
design this port refused — the second of those two rows would be the *only* row,
and the pane would feel broken.

Two closures were considered and neither belongs to a pane port:

| Closure | Why not here |
|---|---|
| nudge the worker whenever the published snapshot changes | the snapshot carries host CPU and memory, so it changes on nearly every tick: a 1 Hz poll becomes a ~100 Hz one. A regression in idle cost bought with latency nobody can see |
| nudge only when the session section changes | probably right eventually, but it is a change to the render loop's contract *plus* a rate policy (without a floor, an agent emitting activity text quickly breaks the spike's own 10 Hz ceiling). It belongs to a change about ADR-V11's frame budget, with its own measurement |

So the honest state is: **the session list can be a plugin, and a plugin's view of
kernel state is one render interval stale.** That is a property of the host, not of
this pane, and every plugin pane already has it — this is simply the first pane
whose content changes fast enough for it to matter.

### The open gaps this port leaves

| Open gap | Where the host stands | Cheapest closure |
|---|---|---|
| **a centred line** (new) | every node draws from the left; `Gauge` right-aligns a suffix and `Fill` can push a run flush right, but neither centres | an alignment on a line node. One consumer so far, so it is recorded rather than designed |
| **a pane's border chrome** (new) | a pane's block is drawn by the host around whatever the plugin returned; nothing describes an overlay on it | this is §9's frame-node row seen from the other side, and its **third** consumer (file-viewer search bar, global search strip, this) |
| the render interval | above | above |
| §8's ellipsizing clip | unchanged; this pane's fitting stays in the kernel | a line that clips with an ellipsis |
| the four vocabulary rows §10 left open | unchanged (frame node, bottom-anchored row, search accent, italic) | this port needed none of them |

### What Phase 4 has established, now that the gate pane is through

Five panes are reproduced and one surface is recorded as structurally unportable.
The catalogue that draws them has grown by four nodes and three style roles across
those five ports, and the last one — the pane the whole model hinges on — needed
none of them. What remains open is not expressiveness: it is **chrome** (a frame
node, an alignment, a bottom-anchored region), **clipping** (§8), and **when a
plugin is asked to render**. The first two are additive vocabulary with two or
three consumers each. The third is the only one that is a design question, and it
is the one this port promotes from a spike's footnote to a measured, named gap.

## 14. The handover that did not happen, and the gate that would have allowed it

§13 closed with the phase's five ports done and named what was left open:
chrome, clipping, and when a plugin is asked to render. The next step is not a
sixth port but the **handover** — stop drawing a native pane, delete its
renderer, let the plugin be the pane — and the info panel was chosen to go first
for the same reason it was chosen for §1: it is a pure display surface, so
nothing about selection, keys or mouse can confound the answer.

The answer is that no pane can be handed over yet, and it is not a fact about the
info panel.

### The blocker is the build, and every link is already enforced

| Fact | Enforced by |
|---|---|
| a bundled pane is a Luau program | `src/plugin/bundled/info-panel/init.luau` |
| running it needs `mlua` | `Cargo.toml`, `plugins = ["dep:mlua"]` |
| `mlua` is optional and not default | `Cargo.toml`, `default = []` |
| the default build must not gain it | the `plugins` CI job asserts `cargo tree --edges normal` shows no `mlua`; it is a required check |
| the *release* must not enable it | `release/workflow-invariants` specifies it; `scripts/dev/lint-workflows.sh` invariant 2 enforces it over `cd.yml` |

So the pane a user installs cannot be drawn by a plugin. Deleting
`src/ui/info_panel.rs` would make `F2` open an empty column on every release
while `cargo nextest run --all --features plugins` stayed green: the failure is
absent from the build that ships and invisible in the build that is tested
hardest. It blocks all seven panes identically, which is why ADR-37 states the
condition once and applies it to every row.

### The gate permitted exactly this, which is the finding worth carrying

`tests/teardown_gate.rs` derived a pane's readiness from two conditions — the
bundled plugin exists, and `src/app/view.rs` no longer names the native renderer
module. The deletion above satisfies both. The row would have been recorded
*handed over*, `recorded_verdicts_match_the_tree` would have **required** that,
and `every_listed_path_survives_until_its_unit_is_ready` would then have stopped
protecting `src/ui/info_panel.rs` — signing off the deletion of a pane no
released binary can draw. That is the silent class the gate's own module note
says it exists to catch, and it is the third time a probe here has had to be
tightened rather than a verdict flipped (§10's write-shaped binding, §11's node
named `Fill`). Three corrections in three ports is the argument for the gates,
and also the argument for never reading one's green as agreement.

The probe now has a third conjunct, read from `Cargo.toml`'s default feature list
rather than from `cfg!(feature = "plugins")` — the `cfg!` answers "was this test
binary built with the feature", which under `--features plugins` is the answer
that permits the deletion.

### Three pane-level requirements the release blocker hides

None is closed here: each is useful only once a plugin pane can reach a user, and
this phase has twice refused to design from one blocked consumer (§7's
`thurbox.format.*`, §10's non-pane extension point).

| Handover requirement | Where the host stands | Cheapest closure |
|---|---|---|
| **the same seat** | `PaneSlot`'s only member is `Right`; the info panel is `RegionId::Info`, its own region with a `Percent(15)` share and a ≥120-column rule. A plugin pane cannot sit there, so its frame is a different rect with a different title | a slot that names an existing region, decided with the layout rather than with a pane |
| **the same toggle and the same flag** | `Action::ToggleInfoPanel` toggles `App::show_info_panel` and `[features] info_panel` gates it; a plugin pane's visibility is `TogglePluginPane` plus a stored per-pane choice. No manifest field asks a pane to answer a kernel action or ride a kernel feature flag | a manifest declaration binding a pane to an existing action and flag — which is also how the `[features]` flags eventually retire |
| **the same latency** | the render worker polls on a fixed 1 s cycle (`PLUGIN_RENDER_SLICE` × `PLUGIN_RENDER_SLICES`). This is §7 and §13's render-trigger gap, and the info panel is its worst case: live CPU and memory gauges plus per-automation countdowns | event-driven render, §13's named gap, with its own rate policy |

The third is the one to weigh before the others. §13 argued the 1 s staleness was
tolerable *because* a plugin pane is a hidden reproduction, so the surface the
user watches is still the kernel's. A handover inverts that argument entirely:
the stale pane becomes the only pane.

### The proposed proof cannot fail

The handover was to be proved by the acceptance snapshots not moving — if the
plugin renders identically, replacing the renderer changes no frame. For this
pane that test is vacuous. There are seven snapshots (`src/app/snapshots/`), all
captured on a welcome screen or a modal with **no active session**, while
`App::render_info_panel` returns early without one and the pane is seated only at
≥120 columns. None contains an info-panel label (`Name:`, `Branch:`, `Agent:`,
`Context:`, `Hooks:`), so they would have stayed byte-identical had the pane been
deleted and replaced with nothing.

The oracle that *can* fail already exists and is what the port relies on:
`tests/bundled_info_panel.rs` asserts tree equality against
`ui::info_panel::info_tree`, and `ui::info_panel`'s own tests paint that tree
against the retained pre-port renderer. The lesson is §6's, one level up: the
audit method there was *read the pane's calls, not the node catalogue*; here it
is *check that the proof could have failed, not that it passed*.

## 15. The first handover attempted on a pane with keys: the tasks pane

§14 attempted the handover on the info panel because it takes no input, and hit a
blocker about the *build*. This section is the attempt on the first pane that
does take input — the tasks pane — and it produces a second, independent blocker
that has nothing to do with the release. Half of the parity work landed; the half
that did not is the half everyone assumed would be easy.

### What landed: the pane's copy scrolls, and it is proved by a frame

§8's table left this pane two geometry divergences. §9 closed the second one's
mechanism for every remaining pane — a list names the row its cursor is on and
the kernel windows it — and this pane never took it up, because the published
task section carried **no cursor index**. Its per-row `selected` flag could not
serve as one: that flag is gated on the cursor being *visible*, so a copy
anchored on it would jump back to row 0 the moment thurbox's own pane lost focus.

Closed by publishing the anchor separately from the appearance (ADR-38), which is
§9's own rule with its second consumer. The consequences worth carrying:

- the claim for this pane is now the file viewer's stronger one — the plugin
  paints the **native frame** at a height where the pane scrolls, not merely an
  equal tree. Tree equality alone could not have said it, because the two trees
  are equal at *every* height once the window moved into the renderer;
- `ui::tasks_panel` consults a width and never a height. The window is resolved
  once, by the renderer, for both panes — two implementations would be two panes
  disagreeing about which rows sit beside the cursor;
- out-of-range split into two cases that are not the same: a cursor past the end
  of a *shortened* list clamps (what the native pane always did), a cursor past
  the *published bound* is not published (what the file section already ruled).

§8's **first** divergence is unchanged and is now the pane's only rendering one: a
title too wide for the column keeps its ellipsis in the native pane and loses it
in the plugin's. That is the ellipsizing-clip row, and it now has three recorded
consumers (§8, §11's hunk header, §13's fitting) without a shipping pane that
needs it.

### What did not land: every key, and the reason is not the editor

Parity was defined as the pane's ten `KeyContext::Tasks` actions. Two are
expressible, eight are not, and the pane is **not** replaced.

| Action | What the native pane does | What a plugin would need |
|---|---|---|
| `TasksNext` / `TasksPrev` (`j`/`k`) | move `task_ui.task_panel_index` | a **view write** — nothing writes a cursor |
| `TasksPreviewDown` / `TasksPreviewUp` | scroll the **central pane's** preview | a view write, plus a surface a right-column pane does not own |
| `TasksNew` (`n`) | create a task, then focus the editor | a create binding, a central seat, a text write |
| `TasksOpen` (`e`/`Enter`) | focus `InputFocus::TaskEditor` | a focus write, plus that seat |
| `TasksRun` (`r`) | a modal whose outcomes are *type a prompt into a running session* or *spawn one* | a modal, plus two powers no capability names |
| `TasksOpenRelated` (`o`) | switch the active session | a view write |
| `TasksCycleStatus` (`Space`) | set the task's status | **nothing new** (ADR-35) |
| `TasksDelete` (`d`) | soft-delete the task | **nothing new** (ADR-35) |

**The finding: the last two do not survive either.** `Capability::TasksWrite`
addresses a task by id and the ids arrive on the published rows, so `Space` and
`d` look portable. They are not, because of *which* row they would act on. A
plugin receives a key only while one of its own panes holds focus
(`InputFocus::PluginPane`); `App::build_tasks_snapshot` marks a row as the
cursor's only while the **native** pane holds focus or a search preview moves it.
Those conditions are disjoint. While the plugin can be pressed at, the kernel
publishes no cursor's row; while it publishes one, the plugin receives nothing.

So the port fails at `j`, not at the editor, and the one-sentence form is worth
memorising: **a plugin pane's keys and the kernel's cursor cannot be live at the
same time.** `tests/tasks_pane_input_gap.rs` keeps that sentence true — one probe
per missing power, in §10's shape, with the record-versus-view-write distinction
ADR-35 forced on the global-search gate written in from the start rather than
retrofitted.

Nothing here was worked around. Declaring `input` plus the two record-write keys
was available and was refused: the pane would answer `Space` against a row it
draws no cursor on, which is a worse pane than one that takes no keys — and
`plugin::keymap` already refuses to publish a binding the host could not deliver,
for exactly that reason.

### The editor and the picker: answered with evidence, and they stay kernel

The brief asked whether a plugin could own the central-pane editor and the
trigger-time picker, or whether they stay kernel like the F1 editor under ADR-V21.
They stay kernel, and each wall is a fact rather than a preference:

| Surface | Walls |
|---|---|
| the central-pane editor | **seat** (`PaneSlot`'s only member is `Right`; the editor is drawn into the centre), **focus** (`InputFocus::TaskEditor` is a view write), **text** (`Capability::TasksWrite` states it grants no creation and no editing, because a task's title and description are authored in thurbox's own editor — so handing the editor over is the write the capability was defined to exclude) |
| the trigger-time picker | **modal** (a manifest declares panes, commands, keys, a service, CLI verbs and spawn env — nothing that owns the interface's input), **reach** (its outcomes are typing into a running session's PTY and spawning a session; the vocabulary names neither, and `AutomationsWrite` — the widest grant defined — is careful to make even *running* a request the kernel fulfils) |

`Capability::Spawn` is not a counter-example: it contributes environment to spawns
thurbox already makes, which is not the power to start one.

### What this means for the phase

Two independent blockers now stand between a reproduction and a handover, and
they are different kinds:

1. **the build** (§14, ADR-37) — no released binary can draw a bundled pane. It
   blocks all seven panes and is one release decision;
2. **input** (this section, ADR-38) — a pane whose keys move a cursor cannot be
   handed to a plugin at all, whatever the build does. It blocks the panes that
   *have* keys, which is five of the seven: tasks, automations, the file viewer,
   the session list, and the code review.

The second is the more interesting one, because §14's three pane-level
requirements (a seat, a toggle binding, event-driven render) are all closable
without changing what a plugin *is*. This is not: a plugin that may move the
user's cursor and take focus is a different security story, and it is the same
wall global search hit from the other side (§10's fourth structural row). So the
honest reading of the phase is now:

- the **read-only** panes (the info panel) are blocked only by the build;
- every pane whose keys do more than draw needs a view-write channel designed
  first — with its own change, its own bounds and its own argument about what an
  installed plugin may do to the interface;
- and `docs/PHASE6-TEARDOWN-READINESS.md`'s handover worklist should be read in
  that order: the release flip, then the view-write design, then the panes.

## 16. The second handover attempted on a pane with keys: the file viewer

§15 attempted the handover on the tasks pane and found a second blocker
independent of §14's build one: a plugin pane's keys and the kernel's cursor
cannot be live at the same time. This section is the attempt on the **file
viewer**, whose whole interaction is keys, and it produces the same verdict for
harder reasons plus one structural fact none of the five ports before it had.

### What landed: the pane's copy grows its scroll track, proved by a frame

§9's table left this pane three open rows. The second — *the scrollbar* — is now
closed, and the record of *why* it was open is worth keeping: the native pane
reserved its rightmost column **outside** the tree (`scrollbar::reserve_track` in
`render_rows`, then a paint into what was left), so the closure was recorded as
"Phase 6's business, not a reproduction's". This is Phase 6, and the objection
answered: put the reservation in the renderer and the native pane's rows land in
the rect they were already painted into, so nothing about the native pane moves.

The shape is ADR-26's trade applied to one column (ADR-39). A list declares
`scrollbar`; the kernel reserves the column, draws the thumb at the declared
cursor, and lays the rows out in the remainder. Three consequences worth carrying:

- **The claim rises to the whole frame.** `tests/bundled_file_viewer.rs` now
  asserts the plugin paints the native frame *including the thumb's column*, and
  asserts it non-vacuously — the same rows without the declaration paint a
  different frame, so a passing comparison is evidence a track was drawn rather
  than that neither pane drew one.
- **It is not inferred from the cursor**, though every list that scrolls has one.
  Three of thurbox's own panes (`tasks_panel`, `automations_panel`,
  `project_list`) draw selectable lists that overflow *without* a scrollbar, so
  inference would have put a track into panes that deliberately have none and
  moved their frames. "A pane that overflows wants a scrollbar" is false in
  thurbox, which is the kind of thing only reading the panes tells you (§6).
- **A plugin's track is an indicator, not a control.** The thumb reports a cursor
  the plugin does not own, so a drag has no destination to write: no scroll target
  is recorded for a plugin pane. That is the missing view write again, from a
  direction nobody had looked from, and it is pinned in the input gate rather than
  left to be discovered.

§9's other two rows are unchanged. The **search bar** stays out of scope, and this
port sharpens why: the bar is drawn *outside* the pane's `Files` block, while a
plugin pane's block is the host's — so even with a frame node the plugin's bar
would sit *inside* its frame and the frames would differ. It is §13's
pane-chrome row (a pane's border chrome is the host's), not a container-node row,
and this is its fourth consumer. The lazily-filled tree is also unchanged.

### The capability the brief expected to widen, and did not

This port was specified as needing a wider `files`. It needs nothing, and the
reason generalises past this pane: **the missing parity is powers, not facts.**

| The section carries | It refuses | Why the refusal survives |
|---|---|---|
| a basename, a depth, an expansion state, a search verdict, a cursor, the nerd-font setting | a path | a path is only needed in order to **act** on a file, and acting is a process launch |
| | a directory's unexpanded contents | expansion is a filesystem read the kernel performs on a keystroke; publishing what nobody expanded makes the tree the publisher's, not the user's |
| | a file's contents | the native pane never shows contents — it opens an external editor — so no parity requirement asks for them |
| | the search query | it is drawn only inside the bar above, which cannot be drawn |

This is ADR-30's finding restated with more evidence: a plugin holding `read_dir`
could draw *a* file tree but not *this pane*. `Capability::Fs` stays undeclared,
reserved by `tests/teardown_gate.rs` for v1's "place a file in an agent's own
config dir" power — adding a filesystem binding here would advance a teardown
verdict as a side effect of drawing a tree.

### What did not land: every key, and this pane has no partial surface

| Action | What it writes | The power a plugin would need |
|---|---|---|
| `FileViewerDown` / `FileViewerUp` (`j`/`k`) | the cursor | a **view write** |
| `FileViewerCollapse` (`h`) | the expansion set, and the cursor when it jumps to a parent | a view write |
| `FileViewerExpand` (`l`/`Enter`) on a directory | the expansion set, **reading the directory** to fill it | a view write *or* a filesystem capability |
| `FileViewerExpand` on a file | nothing in the tree — it **launches an editor process** | a process launch, wider than any capability defined |
| `FileViewerSearch` (`/`) | the query, and the expansion set as matches are revealed | a view write, plus a sub-mode |
| `FileViewerNextMatch` / `FileViewerPrevMatch` (`n`/`N`) | the cursor, and the expansion set | a view write |

**The difference from §15 is the absence of an argument to have.** The tasks pane
had two keys that needed no new host power and failed for a *second* reason
(disjoint focus). Not one file-viewer key is a record write, so there is nothing
to weigh: `not_one_of_the_panes_keys_is_a_record_write` derives that from the
dispatch itself rather than asserting it here.

**And the `/` sub-mode fails a different requirement than the keys do** — the one
that says a ported pane's keys are rebindable and appear in the F1 editor. They
are not, by design: `App::focus_key_context` returns `Global` while
`search_active`, so every character types into the query and the sub-mode's keys
are matched literally. A plugin declaring `input` *would* receive those
keystrokes, and they would search nothing: the search's effect is revealing
matches by expanding directories, moving the cursor between them, and marking
which rows matched — kernel state with no channel inward. That is §10's objection
to porting global search, met again from inside a surface that really is a pane.

### The structural fact that is new: the module is the model

For the five panes ported before this one, the module a handover deletes is a
renderer over records something else owns — `tasks_panel.rs` over `Task` rows,
`project_list.rs` over sessions, `info_panel.rs` over a session's info.
`file_viewer.rs` is not. It holds `FileViewerState` — the roots, the expansion
set, the cursor, the search — `App` owns one as a field, and
`App::build_files_snapshot` reads it. It also owns `visible_window`, the rule
**every plugin list** is scrolled by (ADR-30) and four other native panes window
with.

So for this pane "delete the native renderer" would delete the state the
replacement reads *and* the scrolling every plugin pane depends on. Lifting the
model out of `ui` is deliberately **not** done here: the pane cannot be handed
over even with it moved, so it would be motion without a destination, and the
question "which module owns a pane's model" is better answered in the change that
has a consumer for the answer.

### What this means for the phase

The two blockers §15 named are unchanged, and this port adds a third that is
narrower but real:

1. **the build** (§14, ADR-37) — no released binary can draw a bundled pane; all
   seven panes, one release decision;
2. **input** (§15, ADR-38) — a pane whose keys move a cursor cannot be handed to a
   plugin at all; five of the seven panes;
3. **a pane whose module is its model** (this section, ADR-39) — the deletion
   removes state and shared helpers, not only a renderer. One pane, and it is a
   refactor rather than a design.

The order for `docs/PHASE6-TEARDOWN-READINESS.md`'s worklist is unchanged by that:
the release flip, then the view-write design, then the panes — with this pane's
model move as a step inside its own handover.
