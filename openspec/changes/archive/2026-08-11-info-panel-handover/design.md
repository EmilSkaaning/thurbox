# Design

## Context

Six native panes are reproduced by bundled plugins; none is drawn. The four
handover blockers that stopped three earlier attempts are closed: the seat
(ADR-46), the toggle and feature bindings (ADR-47), the recorded oracles (ADR-42,
ADR-48) and the event-driven render trigger (ADR-49). What still blocks the other
five panes is **focus** — a seated pane is `InputFocus::PluginPane`, so
`KeyContext::SessionList` / `Automations` / `Tasks` do not resolve — plus each
pane's own recorded gap rows.

The info panel is outside both. It declares no `input` capability, has no scoped
keyboard, no cursor, no mutation, and it is the only reproduced pane with no gap
file. So it goes first, and it goes alone.

## Goals / Non-Goals

**Goals.** Delete `src/ui/info_panel.rs` and everything that called it. Leave the
Info column indistinguishable from before at the same widths, with the same toggle
and the same `[features]` switch. Keep the recorded oracle able to fail. Leave the
`--no-default-features` build coherent, even though it loses the pane.

**Non-goals.** No second handover (see "Why the code review is refused"). No new
capability. No focus work. No change to `settings.toml`, the database, or any
dependency.

## Decisions

### The kernel's own occupant is deleted, not switched off

`App::show_info_panel` goes with the renderer. The alternative — keep the `bool`,
initialise it `false`, let `ToggleInfoPanel` keep flipping it — compiles, passes
every existing test, and is wrong in a specific way: `layout_for` carves the seat
when **either** occupant wants it (ADR-46), so a flag nobody paints from still
carves the column. Pressing F2 in a build with no plugin host, or with a broken
`info-panel` plugin, would produce a bordered 15% column containing nothing. That
is precisely the failure `tests/teardown_gate.rs` exists to prevent, reached from
inside the change that was supposed to honour it.

Deleting it makes the seat's flag `self.seat_taken(PaneSlot::CenterLeft)` — a claim
or nothing — so the impossible state is unrepresentable rather than merely untested.

Consequence, stated because it is a behaviour change and not a bug: the panel's
visibility is now **persisted** (plugin pane visibility lives in `metadata`), where
`show_info_panel` reset to `false` every launch. That is the v2 property
`plugin-host/pane-visibility` already specifies, and it is the reason a handed-over
pane is *more* durable than the pane it replaces, not less.

### `default_visible` stays `false`, against the brief

The brief for this work asked for `default_visible = true`. It is `false`, for three
reasons:

1. **It is what the pane did.** `App::show_info_panel` initialised to `false`; F2
   showed the panel. A handover is about which code draws a pane. Changing the
   default screen in the same change would mean any complaint about the new column
   lands on the handover, and any complaint about the handover lands on the column.
2. **No snapshot would catch it.** The seven acceptance snapshots render at
   `SNAP_COLS = 100`, below the 120-column threshold at which the Info seat is
   carved at all — the same fact that made §14's proposed proof vacuous. So seeding
   visible would change every wide install's first launch with nothing failing.
   A default change with no pinned evidence is the combination to avoid.
3. **The exemption permits it, it does not ask for it.**
   `PANES_DRAWN_IN_A_NATIVE_PANES_PLACE` in `tests/bundled_manifests.rs` exists so a
   handed-over pane *may* seed visible; its argument is that "visible **and**
   duplicated" is the mistake, and the handover removes the duplication. It says
   nothing about what the seed should be.

The pane is still added to that list, with `default_visible = false`, because the
list is what a reader consults to learn which bundled panes are no longer
reproductions — and leaving it empty after the first handover would make the next
one look like the first.

### The empty state: accept the plugin's behaviour and pin it

§14 found the one divergence no oracle covers. With no active session
`App::render_info_panel` returned before painting its block, so the seat was a
borderless gap; a plugin pane's frame is painted by the kernel before the tree is,
so it always has a border, and this plugin then draws its System section.

Three options:

| Option | Rejected because |
|---|---|
| **accept the plugin's behaviour** (chosen) | — |
| draw an empty bordered pane with no session | strictly worse than both: a titled box with nothing in it, when the pane holds host CPU, RAM, the data-dir size and the automation countdowns, none of which need a session |
| do not carve the seat with no session | puts a *content* condition into the layout, which no other seat has, and makes the column appear and vanish as the user deletes the last session — plus it would need the kernel to know what the plugin will draw, which `plugin-host/panes` forbids |

So a sessionless launch with the panel shown now has a bordered `Info` column with
System and any upcoming automations. `tests/bundled_info_panel.rs`'s
`with_no_session_the_plugin_still_shows_what_it_knows` already pinned the tree; an
acceptance test now pins the *frame*, which is the half that was missing.

### A click on the pane no longer eats a text selection

Found by driving the handover rather than by a test. `handle_mouse_click` hit-tests
the click registry **before** falling back to `pane_rects`, and `pane_rects` contains
the info panel — which is how drag-selecting text out of it works today. Every
visible plugin pane records a whole-rect `ClickAction::PluginPaneRow` target, so after
the handover a drag in the Info column would be swallowed by a target that then does
nothing: `focus_plugin_pane` refuses a pane whose plugin never declared `input`, and
`offer_click_to_plugin` reads the *focused* pane, so both halves are no-ops for this
pane.

So a pane that cannot receive input records **no** click target. That is not a
concession to this pane; it is the registry agreeing with the two guards that already
exist. Registering a target whose only effect is to consume the click is the bug, and
the handover is what made it observable.

Rejected alternative: record the target and let the selection code run afterwards.
That means a click with two owners, and the registry's contract is first-match-wins.

### The oracle keeps its recordings, byte for byte

`tests/bundled_info_panel.rs` asserted three edges: `native == recording`,
`plugin ≈ native` (legible), `plugin == native` (exact). The first and third name
`info_tree`, which this change deletes. What is left is `plugin == recording`, and
the recordings are **not regenerated** — that is the whole point of ADR-42, and a
`cargo insta accept` in this change would silently convert eleven statements about
the native pane into eleven statements about the plugin.

`Case` therefore stops carrying `SystemMetrics` / `AutomationEntry` (the deleted
types) and carries the published `SystemSnapshot` / `UpcomingAutomationSnapshot`
instead. The `SessionInfo → SessionSnapshot` derivation is kept exactly as it was,
because it mirrors `App::build_pane_context` and that mirroring is what makes an
equality failure a statement about the plugin. The published context is bit-identical
to what it was, which is why the recordings do not move.

`countdown_secs` — the helper that recovered seconds from the native pane's
pre-rendered countdown string — is deleted with the string it decoded.

### `SystemMetrics` moves to the model, not to `session`

It was declared in the pane it fed. Two homes were available: `src/app/metrics_state.rs`,
which owns the value and is filled by the collector, or `src/session/`, where the pane
context's `SystemSnapshot` already lives. `app` wins: the type is an *input* the
collector produces for one consumer, not a shared vocabulary. `SystemSnapshot` is the
type `session` already has for this data, and adding a second one there would leave
two spellings of the same five numbers in the same module tree.

`AutomationEntry` is deleted rather than moved. It carried a pre-rendered countdown
*string* into the native pane; the published snapshot carries seconds and the plugin
formats them, which is the split ADR-29 established.

### `--no-default-features` loses the pane

That build has no plugin host, so it has no info panel. Three things make the loss
coherent rather than a hole: the seat is carved only by a claim and no claim can
exist, so no empty column; `toggle_panes_bound_to` is already a
`#[cfg(not(feature = "plugins"))] -> false` stub, so `ToggleInfoPanel` reaches the
arm that reports what provides the pane; and `[features] info_panel` still gates
that report, so the switch keeps meaning what it says.

Rejected: keep `src/ui/info_panel.rs` under `#[cfg(not(feature = "plugins"))]`.
`migration/phase-4` forbids exactly this ("A port MUST NOT satisfy this by keeping
both renderers and selecting between them on the compile-time feature") and it is a
good rule: it produces two panes that differ by build, and the one users install is
the one nobody tests hardest.

## Why the code review is refused

It was the second pane named for this change. `tests/code_review_pane_handover_gap.rs`
re-derives eleven rows from the tree and **ten are still blocked**. One closed
(`no-central-seat`, by ADR-46). The others are not wiring:

- **two seats.** The diff owns the central pane; the changed-files list owns the
  file-viewer column, which `App::layout_for` forces present while a review is open.
  No slot names `RegionId::FileViewer`, deliberately, so handing over the diff alone
  would leave a native changed-files list beside a plugin diff.
- **its keyboard is not in the keybinding system.** `handle_code_review_key` and
  `handle_review_files_key` are captures keyed on `self.focus`, run before the
  keybinding lookup; `KeyContext` declares no review scope. The keys are not
  rebindable *today*, so no `keybindings.json` could restore them after a handover.
  Turning them into scoped actions is upstream of anything plugin-facing.
- **five operations no capability performs**: `review_comments`/`review_marks`
  writes, `t` (which runs `git diff`/`git show` on a worker), `y`/`e` (clipboard and
  the session's pty), cursor writes for every navigation key, and the resolved width
  `v`/`w`/`←`/`→` need.
- **the mouse channel carries a row, and one click means a column.** On a paired
  side-by-side row the half clicked decides which side a comment attaches to;
  "the user clicked the old side" is not a row, so no additional target kind
  expresses it.

Deleting `src/ui/code_review.rs` would therefore replace a mouse-first, eleven-button,
searchable, retargetable review with a scrollable read-only document. The plugin
reproduces the *document* (ADR-44) and nothing else. §20's ordering stands: the keys
become scoped actions, then the narrow cursor write, then the two seats.

## Risks / Trade-offs

- **A bundled plugin can now fail in a way that removes a pane.** Before, a broken
  `info-panel` plugin cost nothing; now it costs the Info column. Mitigated by the
  action's report naming the plugin, by `thurbox-cli plugin doctor`, and by the pane
  showing `Info (error)` in its own title with the last good tree underneath
  (`paint_plugin_pane`). Not mitigated away: this is what "every pane a plugin" means,
  and it is better surfaced than hidden.
- **A user plugin named `info-panel` shadows the bundled one** — documented override
  behaviour, now with the info panel as the stake. The action's report is what tells
  them.
- **The pre-port byte-identity oracle is gone.** ~600 lines of `#[cfg(test)]` line
  builders that proved the view-tree port painted the pre-port pane cell for cell.
  It cannot be preserved: it needs both `legacy_lines` and `info_tree`, and both are
  in the deleted module. What replaces it is narrower and honest — the recorded tree,
  plus `ui::plugin_pane`'s own renderer tests. The five other panes keep theirs.
- **Latency.** The pane now renders on the `metrics` / `sessions` / `automations`
  source changes at up to 10 Hz (ADR-49), where the native pane redrew on the
  kernel's 250 ms forced-redraw floor. Measured publish rate is ~1 Hz, so in practice
  the gauges tick at their collection cadence exactly as before.
