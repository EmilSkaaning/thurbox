# Design

See `proposal.md` — Why. This records the shape chosen and what was rejected.

## 1. A slot names a region the tree already places

`PaneSlot` grows from one member to five, and gains `seat() -> Option<RegionId>`:

| slot | seat | native occupant today |
|---|---|---|
| `right` | `None` — a column of `RegionId::Plugin(i)` | the tasks panel and file viewer share the column |
| `left` | `RegionId::SessionList` | the session list |
| `left-bottom` | `RegionId::Automations` | the automations pane |
| `center-left` | `RegionId::Info` | the info panel |
| `center` | `RegionId::Center` | the agent terminal / shell / code review |

The mapping lives on `PaneSlot` in `session::plugin_manifest`, beside the
vocabulary it belongs to. `session::workspace_tree` is a sibling of the same pure
layer, and `plugin_manifest` already reaches sideways to
`session::keybindings::KeyChord` for the chord grammar — so this costs no layering
and `tests/architecture_rules.rs` needs no new entry.

**Why one table rather than a `match` in the view.** Five gate probes across five
files read "which seats does a plugin have" out of the source. A single mapping is
a thing a probe can read (`tests/global_search_pane_gap.rs` now asserts no slot
names `RegionId::GlobalSearch`, which is precise and stays true as slots are
added), whereas a `match` scattered through `app` would be five separate reads
that could disagree.

Rejected: **naming the slots after the native panes** (`info`, `session-list`,
`automations`). The name would freeze the pane a seat exists for at exactly the
moment the point is that any pane may sit there — and after a handover
"the info slot" would name the plugin's own pane. Geometric names say where, which
is all the kernel decides.

Rejected: **letting a manifest name a `RegionId` directly**. It would expose
`Header`, `Footer`, `GlobalSearch` and `StatusMessage` as addressable, which is a
much wider surface than the four seats, and the two enums would then have to stay
in step forever. A slot is the plugin-facing vocabulary and a region is the
kernel's; the mapping is the seam.

## 2. Precedence: a visible plugin pane takes the seat

`App::plugin_seat(slot)` returns the first **visible** pane declaring `slot`;
`App::seat_taken(slot)` is the boolean the view branches on. Each of
`render_left_panel`, `render_automations_pane`, `render_info_panel` and
`render_central_pane` returns early when its seat is taken, and
`render_plugin_panes` paints the seated pane into that same rect.

Rejected: **the native pane wins while it exists.** It is the conservative
reading, and it makes the whole change unexercisable: no seat could ever be
occupied until the renderer it replaces was already deleted, so the first handover
would be the first test of the seating — the exact "big-bang" shape every gate in
this phase exists to prevent. With the plugin winning, a user can show a bundled
reproduction today and see it drawn where the native pane was, and hide it to get
the native pane back.

Rejected: **suppressing the kernel action that toggles the native pane.** The
kernel keeps its own pane's `show_*` state, so hiding the plugin pane restores
exactly what was there. Making the plugin steal the toggle would leave the seat
empty and unreachable whenever the plugin pane was hidden or not placed.

**Two panes, one seat.** The first in publication order takes it; the rest are not
placed and are not painted anywhere else. That is the rule the right column already
applies when the centre would starve (`plugin_columns_that_fit`), and it means a
second claimant is a silent no-show rather than an overdraw. Publication order is
stable (discovery order, then manifest order), so the outcome is deterministic.

## 3. A claim is enough to carve the seat

`App::layout_for` ORs the claim into the flag that carves each seat:

- `show_session_list: self.show_session_list || seat_taken(Left)`
- `show_info_panel: self.show_info_panel || seat_taken(CenterLeft)`
- `show_automations_pane: self.features.automations || seat_taken(LeftBottom)`

`center` needs nothing: `RegionId::Center` is in every tree the preset builds.

Without a claim every one of those expressions is what it was, so
`compute_layout` gains no branch, no test in `ui::layout` changes, and the
snapshots cannot move. The seat's *geometry* is therefore the native pane's by
construction rather than by inspection — which is the property §14 asked for
("a slot that names an existing region, decided with the layout rather than with a
pane").

**The lower-left band keeps living inside the left column.** A `left-bottom` claim
does **not** resurrect the left column, exactly as the automations pane does not:
`F9` hides both today, and a claim that carved a column the user had collapsed
would be new geometry rather than the native rule. So a `left-bottom` pane is
visible while the left column is.

## 4. The one content-derived height

`ui::layout::left_column` sizes the lower band `(count + 2).clamp(3, 10)`. A
plugin is never told its rect (ADR-26/29/30/31), so the *kernel* keeps the policy
and supplies the count itself: `ViewNode::stacked_row_count()` — the child count of
the tree's outermost `List` or `Column`, and 1 for anything else.

Rejected: **letting the manifest declare a height.** It is geometry by another
name, and a number in a manifest cannot track a list that grows.

Rejected: **measuring the pane's rendered height** (`ui::plugin_pane::height_of`).
It needs a width, and the width is what the band's own height feeds into — the
band is sized before the rect exists. Counting stacked children is width-free.

Rejected: **a fixed share for a plugin-occupied band.** The native pane grows with
its content, so a fixed share would make the reproduction visibly different in the
one dimension this seat is about.

`stacked_row_count` deliberately does not try to be a rendered height: a paragraph
wraps and a gauge is two rows, neither of which is knowable without the rect. Its
doc says so, so nobody reads it as a measurement.

## 5. What the centre does not get

A `center` pane is drawn with the pane chrome every plugin pane gets — its own
titled block — and the kernel's central chrome is not drawn over it: no
`Agent · Review · Shell` tab strip, no `F9` collapse chevron. Both are painted by
`render_central_pane` on the border of the view it owns, and both address *kernel*
views (`select_central_tab` switches between the terminal, the shell and the
review). Drawing a tab strip whose pills select surfaces that are not on screen
would be worse than not drawing one.

This is recorded as a gap rather than closed here: the review's handover wants the
strip, which needs a tab a plugin pane can appear in, which is a decision about
kernel chrome and not about seating. `tests/code_review_pane_handover_gap.rs`
keeps a blocked row for its second seat, and §21 of the readiness doc names the
chrome.

## 6. Where the types live

- `PaneSlot`, `PaneSlot::seat()` — `src/session/plugin_manifest.rs` (pure data;
  references `session::workspace_tree` and nothing else new).
- `ViewNode::stacked_row_count()` — `src/session/view_tree.rs` (pure).
- `App::plugin_seat`, `App::seat_taken` — `src/app/mod.rs`, with a
  `#[cfg(not(feature = "plugins"))]` `seat_taken` returning `false` so the view's
  guards need no `cfg` of their own.
- Painting — `src/app/view.rs`. `ui::plugin_pane` draws a tree into a rect and does
  not learn which seat the rect is, so `ui` still never references
  `crate::plugin`.

Checked against `tests/architecture_rules.rs`: no module gains a reference it did
not already have.

## 7. Re-verdicting six gate rows

Six rows in five gate files derive their verdict from `PaneSlot == ["Right"]`.
Adding slots makes each probe disagree with its recorded verdict, which is the
gates working: each is re-read, not silenced.

| file | row | after |
|---|---|---|
| `automations_pane_handover_gap` | `no-left-seat` | **closed** — the seat exists and the height policy covers a plugin pane |
| `session_list_pane_handover_gap` | `no-left-seat` | **closed** — same seat |
| `tasks_pane_input_gap` | `no-central-seat` | **closed** as a seat; the editor's remaining blockers (no text write, no modal) are their own rows |
| `code_review_pane_handover_gap` | `no-central-seat` | **closed** — the diff's seat exists |
| `code_review_pane_handover_gap` | `no-second-seat-for-the-changed-files-list` | **still blocked** — probe rewritten: no slot names `RegionId::FileViewer`, which the forced column is |
| `global_search_pane_gap` | `no-band-slot` | **still blocked** — probe rewritten: no slot names `RegionId::GlobalSearch` |

Each rewritten probe reads the `seat()` table rather than the variant list, so it
says what it means ("no slot reaches *this* region") and does not flip the next
time a slot is added for an unrelated reason.
