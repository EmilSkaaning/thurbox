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
pane API would have made it one. Those three sections are the only part of this
document still a worklist.

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
| the scrollbar | a reserved rightmost column with a draggable thumb | nothing | a `scrollbar` field on the list node — the renderer already resolves the window, but the native pane reserves its track *outside* the tree, so moving it in is Phase 6 work |
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
| move a cursor, take focus, restore a snapshot | the kernel-state channel is read-only by construction: every binding under `Sessions`/`Metrics`/`Automations`/`Tasks`/`Files` reads a published snapshot | a write channel — i.e. any installed plugin may move the user's cursor and take focus |

**Vocabulary — cheap, and left open on purpose:**

| Blocked | Where the host stands |
|---|---|
| the strip's bordered block, titled ` Search ` | no frame node, and a pane's frame is the host's (`focus_block` in `App::render_plugin_panes`); §9 recorded the same gap for the file viewer's search bar, so this is its **second** consumer |
| a hint row pinned to the last line under a `Min(0)` list | `Column` stacks children at their natural height from the top; §9's "bottom-anchored fixed-height region" again |
| the search accent (`Theme::search_bar`) | `StyleToken` names no such role, and a plugin may name no colour |
| the italic snippet line under a content match | `TextStyle` carries bold, dim, underline and the selection role |

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
