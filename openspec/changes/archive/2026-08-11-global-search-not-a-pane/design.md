# Design — why global search is recorded instead of ported

## 1. What the surface is, which is what decides the answer

The three ported panes are *rectangles that display kernel state*. Global search
is not one. Read `src/app/search.rs` as a list of what it does and only the first
item is a rectangle:

1. draws a strip (query, per-scope counts, grouped results, hints);
2. **computes** the search — fuzzy metadata over sessions, tasks and automations,
   a substring scan of the active session's file tree, and a debounced scan of
   **every session's live vt100 screen** (`App::session_content_match`);
3. **restyles rows in three panes it does not own** — matched characters accent +
   bold + underlined, unmatched rows muted + dim (`ui::highlight`, applied by
   `project_list`, `tasks_panel` and `automations_panel` from the query
   `App::global_search_query()` hands them through the view);
4. **moves those panes' cursors** as a live preview (`active_index`,
   `task_panel_index`, `automation_panel_index`) and can force a hidden panel
   visible while doing it;
5. **takes focus** on `Enter` and hands it to the owning pane, or **restores all
   of it** from `SearchSnapshot` on `Esc`.

Items 3–5 are the definition of a *mode*: a surface that owns the whole
interface's input and appearance while it is up. Nothing about the node catalogue
is what blocks them.

The claim has to be made precisely, because the loose version is false. A running
search's **verdict** already crosses to a plugin: `TaskSnapshot` carries `dimmed`
and `match_positions`, `FileNodeSnapshot` carries `matched`, and the bundled tasks
and file-viewer panes use them to reproduce v1's dim-and-underline appearance
exactly. So a plugin pane is a pane the search *affects*. The asymmetry is the
finding: the effect flows outward, per row, into the pane that owns those rows —
and nothing flows back, so no plugin can be the surface that *produces* it.

A plugin pane is a rect plus its own keys — the layout seats
it (`PaneSlot::Right`, the only member of a closed set), the kernel-state channel
is read-only ("a binding MUST NOT reach into the running application"), and an
overlay is clipped to its own pane by construction. Each of those is a decision
with a reason attached, and reversing all three is a different concept rather
than a wider vocabulary.

And the record should not be read as "the API is empty here". With `input` plus
the state capabilities a plugin can collect its own query and render its own
filtered list over the published sections — task titles, automation labels, the
open file tree's basenames, the active session's name, agent and repo. It cannot
search the *other* sessions, scan any terminal, restyle a native pane, or navigate
to what was chosen. That is a useful fuzzy picker and it is not global search;
naming the difference is more informative than shipping the picker under the
pane's name.

That is the finding, and it is about ADR-V1's reach rather than about this pane:
**Phase 4's list contains surfaces that are not panes, and the pane model was
never going to express them.** Global search is the first one reached. The
session list will be the second in a milder form (it is a rect, but selecting a
row *is* switching the application's active session), and the code-review view
the third (it owns the central pane, a sub-mode, and a changed-files column in
another slot).

## 2. Rejected: ship the strip's rendering as a pane, fed by a `search` capability

The available port. Add `Capability::Search`, publish `{query, results
(label, snippet, kind), selected}` on `PaneContext`, refactor
`render_global_search` into a `search_tree`, and write a bundled plugin that
reproduces it. Tree equality would have been achievable after rows 5–8 of the
proposal's table were closed, and the deliverable would have looked exactly like
the previous three ports.

Rejected on three independent grounds.

**It would report a capability the host does not have.** The pane would sit in
the right column, 20 columns wide, next to the real strip — searching nothing,
highlighting nothing, previewing nothing and jumping nowhere. Nor could it own
the query: a pane declaring `input` can collect keystrokes while *it* is focused,
but there is no channel by which a query it collected reaches
`GlobalSearchState`, so what it typed would search nothing. Phase 4's own
requirement is
that *a gap worked around by a shortcut a third party could not take MUST be
recorded as still open, because the point of shipping a bundled plugin is to
measure the surface a third party gets*. Here the shortcut is the feature: the
kernel would keep doing all five behaviours and hand the plugin the pixels.

**The published section would be the rendering, not the state.** §8's rule from
the tasks port — *publish a rendering only when two panes must agree about it* —
is what stops a snapshot from drifting into "publish the strings the pane draws".
A search result's `label` and `snippet` are not kernel state a pane interprets;
they are the strip's output, computed for the strip, ordered and capped
(`MAX_PER_GROUP`) and truncated (120 chars) for the strip. A plugin arranging
them proves nothing about a third-party pane, which is the same objection that
killed publishing `"8.0/16.0 GB"` in ADR-27.

**And the capability could not honestly be scoped.** The session scope of the
search reads `parser.screen().contents()` for up to `CONTENT_LINE_CAP` (500)
lines of **every** session — agent output, in full, including whatever a user's
agent has on screen. A capability that let a plugin compute the search itself
would be the widest read in the application, wider than `Files` was ever allowed
to be (`Files` was deliberately narrowed to basenames and named `Files` rather
than `Fs` to avoid advancing a teardown verdict as a side effect of drawing a
tree). A capability that instead published the kernel's *results* avoids that
read and lands in the previous paragraph. There is no third option: either the
plugin reads every screen, or the kernel does the search.

## 3. Rejected: a cross-pane decoration channel

The generalisation of blocker 3: let a plugin publish a query (or a set of
styling hints), and have the native panes consult it — the mirror image of what
the view does today with `App::global_search_query()`.

Rejected because it inverts the one dependency direction Phase 4 exists to
protect. Today plugin state flows *into* a plugin's own pane; a native pane's
rendering depends on kernel state only. With a decoration channel, every native
pane's appearance would depend on the state of every installed plugin — so a
plugin whose own pane is **hidden** could restyle the panes that are visible,
which is precisely the reach that `session::pane_visibility` exists to bound (a
hidden pane must cost nothing and reach nothing). It would also make "which
plugin dimmed my session list" a question with no answer in the interface.

The narrower version — only *one* designated plugin may decorate, and only with
a query — is worse, because "designated" means a privileged plugin, and a
privileged bundled plugin is what this phase measures the absence of.

## 4. Rejected now, deliberately: the cheap widenings

- **`PaneSlot::Bottom` plus a full-width band.** Mechanically small: the
  workspace tree already places `RegionId::GlobalSearch` and `StatusMessage` as
  bands, so seating a plugin band is a region id and a branch. Rejected because
  it closes exactly one of four structural blockers, and the occupant it would
  seat is the pane §2 just rejected. A band also needs a *height*, and the
  strip's is a constant tuned to its content (`GLOBAL_SEARCH_HEIGHT = 12`) —
  a plugin band would have to declare one, which is the geometry the model has
  refused to hand a plugin three times (ADR-26, ADR-29, ADR-30). Whoever needs a
  band should bring the pane that justifies its height.
- **A frame node and a `search_bar` token** (blockers 5 and 7). The strip's chrome is a
  `Block` with borders and a title, both in `Theme::search_bar()` — a palette
  field with no token. A pane's frame is drawn by the *host*
  (`focus_block(&title, FocusLevel::Inactive)` in `App::render_plugin_panes`),
  so a plugin cannot even ask for a different one. §9 already recorded the frame
  node as missing for the file viewer's search bar; this is its second request,
  and the second request is what should trigger designing it — from both
  consumers, not one. The same goes for the bottom-anchored row §9 named: the
  hint line sits on the pane's last row, and `Column` stacks from the top.
- **An italic emphasis** (blocker 8). `TextStyle` carries `bold`, `dim`,
  `underline` and `selected`; the snippet line under a content match is
  `Modifier::ITALIC`. One field, trivially added — and adding it here would put
  a fourth emphasis in the catalogue for a pane that is not being shipped, which
  is how vocabularies grow items with no consumer.

The pattern in all of them: a vocabulary gap is cheap and is worth closing *when
a pane needs it to ship*. None of these does.

## 5. What would make the surface reachable: a provider, not a pane

Recorded because it is the useful half of the finding, and deliberately not
designed here.

Invert the port. Global search's *surface* is kernel-owned by nature — it is
docked chrome, it owns input, and it edits other panes — and none of that is
work a third party should want to do. What a third party plausibly wants is to
**contribute a scope**: search my notes, my open PRs, my shell history, and have
the kernel's strip show the results grouped alongside the built-in ones and
navigate to what was chosen.

That shape asks the host for three things the pane model does not have, and each
is narrower than its pane-model equivalent:

| Provider needs | Instead of |
|---|---|
| a hook called with the query, returning results as *data* | reading every session's screen to compute them itself |
| a result carrying an opaque **target token** the kernel resolves | a write channel into focus and other panes' cursors |
| nothing at all about the strip | a band slot, a frame node, a token and an emphasis |

It also composes with what exists: the shape is much closer to the command
registry (host-invoked, name-addressed, manifest-declared) than to a pane, and a
provider that returns data cannot dim a pane it does not own.

Not designed now for a stated reason: two of the remaining Phase 4 surfaces
(code review, the session list) are also not plain panes, and a non-pane
extension point designed from one consumer is the mistake §7 warned about for
`thurbox.format.*`. The right time is when the phase has all three measurements.

## 6. Why the record is a test and not only a paragraph

`tests/teardown_gate.rs` states the reason already: *a verdict written in
markdown is a fact about a build that expires without telling anyone*. A
"global search cannot be a plugin" paragraph is exactly such a fact — it stops
being true the moment someone adds a bottom slot or a write binding for another
reason, and nothing would say so.

So each blocker gets a probe that reads the tree the way a human auditor would,
and the recorded verdict must agree with it. The probes are chosen to flip on the
*change that matters*, not on incidental edits:

| Blocker | Probe | Flips when |
|---|---|---|
| not a pane slot | `PaneSlot`'s declaration lists only `Right` | a band/bottom slot is added |
| no query or results | the capability vocabulary names no search-shaped capability, and the module table inserts no search binding | a `search` capability appears |
| no cross-pane styling | the three native panes apply `ui::highlight` themselves, and `ui::plugin_pane` mentions neither a query nor a highlight | the plugin renderer learns about a search |
| read-only state channel | `plugin::capabilities` inserts no writer-shaped binding | a write channel is granted |
| chrome, anchoring, italic | the node catalogue declares no frame node, `StyleToken` no search-bar token, `TextStyle` no italic | the vocabulary is widened |
| no plugin claims it | `src/plugin/bundled/global-search*/` does not exist | someone ports it without reading this |

A probe over a *declaration* (`block(…, "pub enum PaneSlot")`) rather than a
whole file is copied from the teardown gate for its reason: an unrelated mention
elsewhere in the file must not flip a verdict.

The alternative considered and rejected: put these rows in `teardown_gate.rs`
next to the pane rows. Rejected because that gate answers a different question —
*may this file be deleted* — and its `global-search-plugin` row is already
correct and must stay blocked either way. Merging the two would make one table
answer "is deletion safe" and "is a port possible", and a failure would not say
which.

## 7. What this change does **not** claim

- It does not claim the strip's *rendering* is inexpressible. Rows 5–8 are four
  small additions away, and the record says so. What is inexpressible is the
  surface.
- It does not claim a tree-equality oracle was attempted and failed. None was
  written: `render_global_search` paints ratatui directly, and refactoring it
  into a `search_tree` is the first step of the port that is not happening.
  Rows 5–8 come from reading the renderer's ratatui calls — which §6 named as the
  method that finds what reading the node catalogue misses — not from a failed
  comparison.
- It does not weaken any Phase 4 requirement. The phase's "report whether the
  host surface sufficed" requirement is what this change satisfies; the added
  requirements say what that report must contain when the answer is *no surface
  would have sufficed*.
