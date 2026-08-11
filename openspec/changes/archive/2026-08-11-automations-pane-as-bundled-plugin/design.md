# Design

## 1. Where the pane sits, and why the reproduction cannot sit there

The native automations pane is placed by `ui::layout::left_column`: the left column
splits into the session list (`Fill { min: SESSIONS_MIN_ROWS }`) and the automations
pane (`Cells(rows)`), where `rows = (automation_count + 2).clamp(3, 10)` and the
whole pane is dropped when the column cannot spare it. Plugin panes are placed by
`LayoutParams::right_regions`: one `Percent(20)` full-height column each, after the
tasks panel and the file viewer, and `PaneSlot` names exactly one slot — `right`.

So the reproduction is placeable **as a pane** and not placeable **where this pane
is**. The four things a `left` slot needs, none of which this change makes:

| Needed | Why it is not a one-liner |
|---|---|
| a second `PaneSlot` variant | cheap; it is the other three that follow from it |
| a plugin region inside `left_column` | the left column is currently a two-child split with a content-derived middle; a third child changes the arithmetic that `automations_pane_appears_below_sessions` and four sibling tests pin |
| a `RegionId::Plugin(i)` index space that spans columns | `PanelAreas::plugin_panes` is collected by `map_while` over contiguous indices and `App::render_plugin_panes` `zip`s it against the visible-pane list in publication order. Split across two columns, that zip pairs a pane with another column's rect |
| a **height policy** for a left plugin pane | the native pane's height is a function of its row count. The kernel does not know a plugin pane's row count at layout time except by counting its last tree's children — which makes the layout depend on plugin output, a coupling no other pane has |

The last one is the real content of the finding: the left column is the one place in
thurbox where a pane's **geometry is derived from its own content**, and the plugin
protocol is built the other way round (a plugin is never told its size, and the
kernel never asks it for one). A `left` slot is therefore a design decision about
whether plugin content may size a region, not a vocabulary addition.

**Rejected: widen the slot vocabulary in this change.** It is a change to the file
that owns every pane's geometry, gated by ~40 layout tests and ~115 pinned frames,
and it would arrive in the same commit as the port that is supposed to be evidence
about the pane. A port that also rewrote the layout would tell us nothing clean
about either.

**Rejected: place the reproduction in the right column and say nothing.** It would
read as a complete port. The pane is in the right column *because that is the only
slot that exists*, which is a fact about the host, and
`the_pane_cannot_be_placed_where_the_native_one_sits` asserts a manifest naming
`left` is still refused — so when someone adds the slot, the test fails and points
at the finding.

**Consequence accepted:** the reproduction's rows are identical and its placement is
not, so the equality claim is about the pane's *content*. That is the same shape as
the file viewer's search bar (out of scope, named) and its scrollbar (chrome outside
the tree) — with placement, one level further out.

## 2. The summary: parts, not a string

A row's tail is `<schedule> · <action> · <when>`:

| Part | Where it comes from | Can a plugin compute it? |
|---|---|---|
| schedule | `once`, or `humanize_cron(expr)` (`daily 09:00`, `weekdays 08:30`, `Mondays 07:00`, `hourly :15`), falling back to the raw cron expression | no — it is thurbox's own cron vocabulary, shared with the automation editor |
| action | `send` / `spawn` / `exec` | yes, given the wire name |
| when | `disabled`, else a countdown (`due`, `in 45s`, `in 2m 30s`, `in 3h 20m`), else `—` | only given a duration; a VM has no clock |

So the section publishes the resolved **schedule label**, the action's **wire name**,
`enabled`, and `due_in_secs` — and the plugin composes the separator, the ordering
and the three-way `when` precedence.

**Rejected: publish the finished summary string.** It is the cheaper change and it
is what `src/session/pane_context.rs`'s module documentation already refuses in
general terms: "Publishing `"8.0/16.0 GB"` would make a pane plugin an arrangement
of strings the kernel formatted, which would prove nothing about what a third-party
pane can do." The summary is the most information-dense element of this pane; if the
kernel composes it, the port measures nothing except that a plugin can concatenate.

**A rule that needed sharpening to reach that answer.** ADR-29 states the line as
"publish the rendering only when two panes must agree about it", and
`format_automation_summary` *is* shared by two native surfaces — this pane and the
`Ctrl+P` automations list modal. Read literally, the rule says publish it. The rule's
purpose, though, is to stop a plugin from re-deriving a mapping whose drift would be
**invisible**: `StyleToken::for_status` is published because a plugin's status dot
would silently disagree with the session list, which no test compares it to. Here the
second consumer is a modal a plugin cannot reproduce, and the plugin's composition is
compared against the native rule on every run of the equality test — so drift is
loud. The rule as it should be stated: *publish a rendering when a plugin's copy of
it would be unchecked*. Both native surfaces keep calling one Rust function, so they
still cannot disagree with each other.

**Where the one rule lives.** `ui::automations_panel::row_summary(schedule, action,
enabled, due_in_secs)`. It moves out of `app` because it is presentation and because
the tree builder that consumes it lives here — and being in `ui` makes it reachable
from an integration test, which is what lets the test compare the plugin against
thurbox's rule rather than against a third copy hand-written in the test. `app`'s
`format_automation_summary` becomes the adapter that resolves the parts from an
`Automation` and calls it, so the list modal is unchanged.

**`format_countdown` moves to `ui` and takes seconds.** It was `pub(super)` in
`app::view`, unreachable from `ui`. Seconds rather than milliseconds because that is
the granularity it displays at and the granularity the snapshot publishes, so the
plugin's copy and the native one are formatting the same number. The three call
sites divide, which is exactly what the function did internally.

## 3. The anchor and the appearance are two facts

This pane windows to the cursor's row **always** and highlights it only when the pane
is focused or a global search is previewing a row here. The file viewer conflated the
two (it always draws its cursor), so one published index served both.

The section therefore publishes `cursor: Option<usize>` — the row to scroll to — and
`cursor_visible: bool` — whether it is drawn. The plugin passes the first to
`ui.list`'s selected row and gates the selected appearance on the second.

**Rejected: one published flag per row (`selected: bool`) and no anchor.** That is
what the task section carried *before* ADR-38 gave it an anchor, and it is the exact
shape ADR-38 corrected: without an anchor the plugin's copy stops scrolling the
moment the native pane loses focus, which for this pane is most of the time.

**Rejected: mirror the task section exactly — a per-row `selected` *and* a
section-level `cursor`.** That is the shipped encoding next door, so this needs a
reason rather than a preference. For the task section the per-row flag is doing real
work: `App::build_tasks_snapshot` computes it per row and a future publication could
in principle mark a row other than the anchor's. Here it would be strictly derived —
`selected == cursor_visible && i == cursor` for every row — so publishing it per row
would be one fact in `n + 1` places, and a malformed publication could highlight one
row while scrolling to another. `cursor` + `cursor_visible` cannot express that
disagreement at all. The cost of diverging from the neighbour is that a plugin author
reading both sections sees two encodings of the drawn cursor; `thurbox.d.luau` says
which is which and why, and the *plugin-visible* consequence is one line either way.

This is the second independent confirmation of ADR-38's split — the anchor belongs to
the list, the appearance to the run — and it arrived from a pane whose two answers
genuinely differ, rather than from a rejection argued in the abstract.

## 4. Module ownership, against the architecture allowlist

| New/changed type | Module | Allowed by `tests/architecture_rules.rs` |
|---|---|---|
| `AutomationRowSnapshot`, `AutomationsSnapshot`, `MAX_AUTOMATION_ROWS`, `UpcomingAutomationSnapshot` (renamed) | `session::pane_context` | `session` references nothing; these are plain data |
| `automations_table`, `upcoming_automations_table` | `plugin::kernel_state` | `plugin → session` only |
| `AutomationRow`, `resolve_rows`, `automations_tree`, `row_summary` | `ui::automations_panel` | `ui → session` (+ `app` for view state, already used) |
| `format_countdown` | `ui` | moved *out* of `app`, so one edge fewer |
| `build_automations_snapshot`, `automation_schedule_label` | `app` | `app` imports everything |

No new edge, so the allowlist, `CLAUDE.md`'s module table and
`docs/CONSTITUTION.md` are untouched. The one place that must see both
`ui::automations_panel` and `plugin::PluginHost` is `tests/bundled_automations_panel.rs`
— an integration test, outside the library's module graph, exactly as the three
previous ports did it.

## 5. What this port measures about the API

Recorded here because the measurement is the deliverable, and it goes in
`docs/PHASE4-PANE-READINESS.md` §17:

- **The view tree needed nothing.** A `list` of `line`s, `accent`/`muted`/`secondary`
  tokens, `bold`/`dim`/`underline`, `selected`, and the list's anchor — all of it
  already there. Six ports in, the catalogue has stopped growing.
- **The state channel took a sixth section** with no new mechanism and no new gate,
  and for the first time an existing capability grew a **second reader** rather than a
  new capability appearing. "Reads your scheduled automations" is one sentence for an
  install prompt whether the plugin wants the due ones or all of them.
- **The formatter case is now made twice.** `formatDueIn` in the info-panel plugin and
  the countdown in this one are the same function, character for character, in two
  bundled plugins — which is the first evidence for §7's `thurbox.format.*` table that
  is not a prediction. Still not added here: it would edit a shipped plugin in the
  change that measures the need, and the right design is a table shaped by three or
  four panes, not two.
- **Geometry: §8's first row is still open, and this pane is the one that wants it.**
  The native pane fits a name into `width − prefix − tail` with an ellipsis. The
  plugin draws the whole name and the renderer clips, so a narrow column loses the
  ellipsis *and* the summary tail. The closure is unchanged from ADR-29's statement —
  an ellipsizing clip plus a flush-right run — and it is pinned by its own test.
- **The layout, §1 above.** The first hard "no" from the host in the phase, and it is
  not about drawing.
- **Input and the write seam sufficed, and §6 is what that cost.** Five of the pane's
  seven keys ship, addressed through `automations-write`, which had no consumer at all
  before this change. What the host still does not tell a pane is whether *it* holds
  focus.

## 6. The keys: what ports, what does not, and who owns the wrap

ADR-38 recorded the tasks pane's keys as kernel-owned, and one of its two reasons was
that "the input path and the cursor path are disjoint": a plugin receives keys only
while its own pane is focused, and the kernel marks a task row as the cursor's only
while the *native* pane is focused. Read as a statement about the kernel's cursor it
is exactly right, and `tests/tasks_pane_input_gap.rs` keeps it true. Read as a
statement about panes in general it is too strong, and this port is where that shows.

**The correction: a plugin pane's cursor is the plugin's own.** A VM persists across
renders, so a cursor is an ordinary module-level local. `onKey` moves it, `render`
hands it to `ui.list` as both anchor and appearance, and the row the user is looking
at is the row the plugin drew its own cursor on. Nothing about that needs a view
write: the plugin is not moving *thurbox's* cursor, it is drawing its own pane. The
published `cursor` is still what it starts from — `nil` until the first key, so a
pane nobody has driven reproduces the native one exactly, which is what keeps the
equality test meaningful.

So the pane's keys divide by *what the key needs*, not by whether it is a key:

| Key | Native effect | Ported | Why |
|---|---|---|---|
| `j` / `k` (+ arrows) | move the cursor | **yes** | the plugin's own cursor |
| `Space` | toggle enabled | **yes** | `setAutomationEnabled(id, …)`, on a *drawn* cursor only |
| `r` | run now | **yes** | `runAutomation(id)` — marks it due |
| `d` | delete | **yes** | `deleteAutomation(id)` |
| `n` | create one | no | the write seam has no creation binding, by construction |
| `Enter` / `e` | central-pane editor | no | a seat `PaneSlot` does not offer, a focus a plugin cannot take, and text authoring `automations-write` excludes |

**`r` stays a request, and this is the change that gives that property a consumer.**
An automation's action may be `Exec`, so `runAutomation` can cause a shell command the
*user* authored to run. What bounds it: the binding writes "due now" and returns; the
kernel's own pass fires it, on the kernel's thread, exactly as it fires a schedule that
came due by itself. No plugin thread executes anything, and a plugin can neither author
nor edit an automation, so the set of programs reachable is the set already scheduled.
Asserted end to end rather than restated.

**Rejected: `Space`/`r`/`d` acting on the kernel's published cursor.** It reads as
tighter coupling to thurbox and it is worse: the kernel's cursor is wherever the native
pane's was, so a user driving the plugin's pane would toggle a row they are not looking
at. That is the shape ADR-38 was right to refuse.

**Rejected: declaring the two unportable keys anyway, wired to nothing.** The host
refuses to register a binding it cannot deliver; a plugin shipping a key that does
nothing would be the same failure one layer up.

**Rejected: one binding per chord for the movement pair.** A `KeybindingDecl` carries
one chord where a kernel `Action` carries a list, so `j` and `Down` cannot both be
declared for one binding id. The plugin declares the letter (which is what the F1
editor then shows and rebinds) and handles `up`/`down` as raw key names in `onKey`,
which is the mechanism the host already offers for a key nobody declared. Recorded as a
finding rather than worked around silently: the natural closure is `chord` accepting a
list, and it is a manifest change, not a pane one.

### Who owns the wrap

The native pane and the session list are one continuous circular list: `j` past the last
session drops into the automations pane at its first row, `k` above the first session
lands on its last, `j` past the last automation loops to the top of the session list,
and `k` at its first row returns to the last session. Every one of those four is
`App::act_session_list_next` / `automations_pane_move_down` and friends assigning
`self.focus` — **view state**, which the kernel-state channel is read-only about and no
capability writes.

**The answer: the kernel owns the wrap, in both halves, and a plugin pane is a discrete
focus stop.** The plugin's contribution is the half it can honestly make — at its first
or last row it **declines** the key, which is precisely "I could not move" and is the
one thing `onKey`'s return value is for. What is missing is the kernel's half: an
unconsumed key in a plugin pane falls through to the binding lookup in
`KeyContext::Global` (`App::focus_key_context` maps `InputFocus::PluginPane` there), and
no global action means "leave this pane downward". So the key does nothing, visibly.

That is the right answer rather than a hole, for a reason that comes from §1: **a wrap
is a claim about adjacency, and adjacency is layout.** The two native panes wrap into
each other because they are stacked in one column and read as one list. The plugin's
pane is in the *right* column, beside the file viewer — a `j` there that jumped the
cursor into the left column would be a lie about what is on screen. The wrap becomes
expressible when the pane can sit where the native one does, which is the same
unfinished decision, and it needs the same thing plus one: a plugin-pane key context
with a kernel action for leaving a pane by direction (`Esc` already leaves, but not
directionally).

**Rejected: the plugin wraps its own cursor (last → first).** It would make `j` at the
bottom do something rather than nothing, and it would be a behaviour the native pane
does not have, shipped under the word "parity". A reproduction that invents an edge case
is worse evidence than one that declines and says so.

**Rejected: give the kernel a "leave the pane downward" action now.** It is a focus rule
for every plugin pane, decided by a pane that is not seated where the rule would matter.
Recorded with its cost instead, and pinned: `the_wrap_out_of_the_pane_stays_kernel_owned`
asserts the plugin declines at both edges *and* that the kernel offers nothing that
completes it, so adding either half fails the test.

### What the host does not tell a pane about itself

The native pane draws its cursor only while focused (or previewed). The plugin cannot
reproduce that gating for *its own* pane, because `render(paneId)` is told nothing about
focus — the published `focused` fields describe the **native** pane being reproduced,
which is the right thing for a read-only copy and the wrong thing for a pane with keys.
Consequences, both accepted and enumerated:

- once the plugin has a cursor of its own it draws it whether or not its pane is
  focused, like the file viewer's native pane does;
- it cannot learn that focus *left*, so the highlight persists after `Esc`; and
- **a write is refused until that cursor exists.** With `cursorVisible` false —
  which is the real state while the plugin's pane is focused — a `Space` would
  toggle whichever row thurbox's cursor was left on, invisible to the user. The
  plugin declines instead, so `j`/`k` places a visible cursor and the row acted on
  is the row highlighted. This is the one interaction that differs from the native
  pane, and it is a refusal rather than a guess.

**The closure is one published fact, and the mechanism already exists**:
`session::pane_visibility` publishes a per-pane boolean into a process-wide slot that the
render worker reads with no signature change, and a `pane_focus` sibling would be the
same shape. Not done here because it changes what *every* pane is told — a host change
arriving inside a pane port, which is the thing the previous five ports were careful not
to do.
