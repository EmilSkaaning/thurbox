# The automations pane becomes the plugin, and the native renderer is deleted

## Why

`tests/automations_pane_handover_gap.rs` recorded ten rows. Four are already closed
(the seat, ADR-46; the focused border and the render trigger, ADR-51/ADR-49; the
fitted name, ADR-55). Of the six that remain, **five stop being requirements** the
moment the pane takes ADR-51's route rather than holding its own keys — and the gate
says so in each row's own words, because it was written to measure the other route:

| Row | On the kernel-keyboard route |
|---|---|
| `central-seat-follows-the-native-focus` | the pane is focused as `InputFocus::Automations`, which `App::render_central_pane` **does** name — so the editor and the run history appear |
| `no-creation-operation` (`n`) | the kernel performs `AutomationsNew` |
| `no-authoring-operation` (`Enter`/`e`) | the kernel performs `AutomationsOpen` |
| `wrap-out-of-the-pane-is-unowned` | both ends of the wrap are kernel focuses, so the kernel's own `j`/`k` handlers complete it, as they always did |
| `pane-is-not-told-its-own-focus` | the published `focused` / `cursor_visible` **are** this pane's focus once it is the interface's automations pane |

Nothing is granted to close any of them. The shipped plugin declares `input` and
`automations-write` and five bindings of its own; on this route it declares neither,
because the keys were never the plugin's to hold.

What is left is **one** row, `the-module-is-a-model-too`, and it is a relocation
rather than a decision: `ui::automations_panel::row_summary` composes
`<schedule> · <action> · <when>` for this pane **and** for the `Ctrl+P` list modal
(`app::automation::format_automation_summary` calls it), so deleting the module would
delete the modal's summary.

## What Changes

- **`src/ui/automations_panel.rs` is deleted** (~700 lines, including the pre-port
  span renderer retained as the view-tree port's byte-identity oracle). The band
  beneath the session list is `src/plugin/bundled/automations/init.luau`, drawn from
  the existing `left-bottom` seat, gated by `[features] automations`, declaring
  `key_context = "Automations"`.
- **The plugin stops holding keys.** Its manifest drops `input`,
  `automations-write` and its five `[[keybindings]]`, and `init.luau` drops its own
  cursor, `onKey` and `onClick`. All seven `KeyContext::Automations` actions resolve
  against the kernel's state while the pane holds focus — `j`/`k` move the cursor and
  wrap into the session list, `Space` toggles, `r` runs, `d` deletes, `n` creates,
  `Enter`/`e` opens the central editor — still rebindable in F1. **This is a
  reduction in what an installed plugin may do**, and it is the point: the pane's
  reach goes from four capabilities to two (`render`, `automations`).
- **`row_summary` moves to `src/ui/automations_list_modal.rs`**, beside the surface
  that still composes it. `format_countdown` stays in `ui/mod.rs`, where it already
  lives for this reason.
- **`show_automations_pane` becomes a claim, not a flag.**
  `layout_for` reads `self.seat_taken(PaneSlot::LeftBottom)` instead of
  `self.features.automations || …`. ADR-50's rule, for its reason: a flag nobody
  paints from still carves a band.
- **The pane seeds `default_visible = true`**, which is the first bundled pane to do
  so — the native band was always on screen, and a handover changes which code draws
  a pane, not whether it is on screen. `tests/bundled_manifests.rs`'s
  handed-over-pane rule is generalised from "seeds hidden" to "seeds at the native
  pane's default", which is what its own doc anticipated; a pane that seeds *visible*
  needs no toggle action, because nothing has to reveal it.
- **The band arrives a moment after the first frame.** This is the first
  always-visible handed-over pane, so it is the first time the spike's predicted
  pop-in is visible: the host starts detached and a pane does not exist until it
  arrives, so the left column is the session list alone for that moment and then
  splits. Recorded as the accepted cost, with the alternative (blocking the first
  frame on a VM) refused by `plugin-host/panes`.
- **The left column's circular wrap is decided and recorded**: it stays the
  kernel's, in `act_session_list_next`/`prev` and
  `automations_pane_move_down`/`up`, and its gate changes from `features.automations`
  to "a pane provides the automations list". Both ends are kernel focuses whoever
  draws either pane, which is what makes the wrap survive **both** left-column
  handovers rather than needing an owner.
- **Global search's automation jump reports when there is no pane**, mirroring the
  task jump (ADR-53).
- **The oracle is rewritten against its recording.**
  `tests/bundled_automations_panel.rs` loses the `automations_tree` / `resolve_rows`
  sides ADR-42 predicted; the thirteen `.snap` files become the expectation and are
  **not regenerated**. Its five key tests go with the keys they measured, and
  `the_plugin_composes_the_summary_thurbox_composes` **stays**, now against
  `ui::automations_list_modal::row_summary` — the one edge that is not differential,
  because that rule survives the deletion.
- **The teardown gate re-verdicts the row and moves its worked example.**
  `automations-plugin` becomes the third `ready` pane row; `EXAMPLE_BLOCKED_PANE`
  moves to a pane the interface still draws.
- **`tests/automations_pane_handover_gap.rs` is retired**, its rows preserved as a
  table in ADR-56 — because none of the five powers it named was granted.

## Non-goals

- **No new capability, no new node, no new binding, no new seat.** Every host power
  this pane needs already exists. The change *removes* two capabilities from a
  shipped manifest.
- **The automation editor, its run history and the `Ctrl+P` list modal stay
  kernel.** They are not panes — no seat, no slot, nothing a manifest could claim —
  and they are reached through the same focus as before, which is precisely what
  ADR-51 buys: the pane holds `InputFocus::Automations`, so
  `App::render_central_pane`'s branch fires unchanged.
- **The session list is not handed over here.** Its own gate keeps three drawing
  rows and a module that is the kernel's navigation model.
- **`--no-default-features` loses the automations pane**, deliberately, and with it
  the central automation editor and run history — the pane is the only door to
  `InputFocus::Automations`. `thurbox-cli automation` is unchanged, the TUI still
  *fires* schedules, and `Ctrl+P`'s list modal and overlay editor still author them,
  so authoring is not lost, only its in-pane surface. `plugins` is in the default
  feature set, so no install is in this position.
- **The empty-state line keeps naming `Ctrl+N`.** It is a rebindable chord printed
  from a plugin, which is the defect the tasks pane's hint row was moved into the
  kernel to avoid — but that line is *inside* the pane's rows rather than beside
  them, and both panes have drawn it from the published `focused` flag since the
  port. Moving it into seat chrome would change the pane's content in the change
  that claims it does not.

## Impact

- Affected specs: `migration/handover` (two MODIFIED, two ADDED),
  `migration/phase-4` (four MODIFIED). `migration/teardown` needs none: its
  worked-example rule already requires the example to move in the change that hands
  its pane over, and this change complies.
- Affected code: `src/ui/automations_panel.rs` (**deleted**), `src/ui/mod.rs`,
  `src/ui/automations_list_modal.rs`, `src/app/view.rs`, `src/app/mod.rs`,
  `src/app/automation.rs`, `src/app/key_handlers.rs`, `src/app/search.rs`,
  `src/app/acceptance.rs`, `src/plugin/bundled/automations/plugin.toml`,
  `src/plugin/bundled/automations/init.luau`,
  `tests/bundled_automations_panel.rs`, `tests/bundled_manifests.rs`,
  `tests/teardown_gate.rs`, `tests/automations_pane_handover_gap.rs`
  (**deleted**), `tests/session_list_pane_handover_gap.rs` (a shared-row reference).
- Docs: `docs/ARCHITECTURE.md` (ADR-56), `docs/PHASE4-PANE-READINESS.md` §31,
  `docs/PHASE6-TEARDOWN-READINESS.md`, `CLAUDE.md`.
- No schema change, no new dependency. `settings.toml`'s `[features] automations`
  keeps its name and meaning; it now gates a pane the manifest binds it to, and
  still gates the TUI's firing and the `Ctrl+P` surface.
