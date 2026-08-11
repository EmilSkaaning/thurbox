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

**Rejected: one published flag per row (`selected: bool`), as the task section
carries.** It is sufficient there because the task section has no anchor at all — the
tasks plugin's list does not scroll — but here it would lose the anchor for an
unfocused pane, and the plugin's copy would stop scrolling exactly when the native
pane still does.

**Rejected: publish both a per-row `selected` and a section-level anchor.** Two
representations of one fact that can disagree; a malformed publication could highlight
one row and scroll to another. `cursor` + `cursor_visible` cannot.

This is the second independent confirmation of ADR-30's split — the anchor belongs to
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
`docs/PHASE4-PANE-READINESS.md` §10:

- **The view tree needed nothing.** A `list` of `line`s, `accent`/`muted`/`secondary`
  tokens, `bold`/`dim`/`underline`, `selected`, and the list's anchor — all of it
  already there. Four ports in, the catalogue has stopped growing.
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
