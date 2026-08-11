# The info panel becomes the plugin, and the native renderer is deleted

## Why

Phase 4 reproduced six native panes as bundled plugins and handed over none of
them. Four handover requirements were closed one at a time — the build (Stage B,
ADR-40), the seat (ADR-46), the toggle and the feature flag (ADR-47), the render
trigger (ADR-49) — plus the oracle (ADR-42, ADR-48). `docs/PHASE4-PANE-READINESS.md`
§24 records the consequence: **all five of §14's requirements are closed**, and what
remains blocking the six panes is *focus* and each pane's own recorded rows.

The info panel is the one pane for which neither of those applies. It takes no
input, so it has no scoped keyboard to resolve and no cursor to write; it is the
only reproduced pane with **no gap file at all** (the other five each have one:
`tests/tasks_pane_input_gap.rs`, `tests/file_viewer_pane_input_gap.rs`,
`tests/automations_pane_handover_gap.rs`, `tests/session_list_pane_handover_gap.rs`,
`tests/code_review_pane_handover_gap.rs`). Its handover is therefore not blocked by
anything, and every change since §14 was made for it.

So this is the first pane thurbox does not draw. `src/ui/info_panel.rs` is deleted:
2018 lines, including the pre-port line builders retained as the port's
byte-identity oracle. What draws the Info column afterwards is
`src/plugin/bundled/info-panel/init.luau`, in Luau, through capabilities a third
party can declare.

## What Changes

- **The bundled pane binds itself to the pane's identity.** `plugin.toml` gains
  `toggle_action = "ToggleInfoPanel"` and `feature = "info_panel"` (ADR-47's
  fields, exercised until now only by tests), and its title becomes ` Info ` — the
  native pane's title, since it is no longer a copy sitting beside it.
- **`src/app/view.rs` stops drawing it.** `App::render_info_panel` and the `info_panel`
  import go; `render_plugin_panes` already paints the `center-left` seat.
- **The kernel's own occupant of that seat is deleted, not left switched off.**
  `App::show_info_panel` goes with the renderer, so the seat is carved by the
  claim alone (`layout_for`'s `show_info_panel` becomes
  `self.seat_taken(PaneSlot::CenterLeft)`). A `bool` that no longer decides
  anything would be the empty column this phase's gate exists to prevent: a build
  that carves a seat nothing draws.
- **`ToggleInfoPanel` reports when nothing provides the panel.** With no kernel
  pane left, the action's own arm has nothing to flip — so it says which plugin
  owns the pane instead of doing nothing silently. That is the honest surface for
  a failed bundled plugin, for a build with no plugin host, and for a user whose
  own `info-panel` plugin shadowed the bundled one.
- **`SystemMetrics` moves to its owner.** The type was declared in the pane it fed;
  `App::metrics_state` owns the value and the collector fills it, so it moves to
  `src/app/metrics_state.rs`. `AutomationEntry` is deleted outright — it existed to
  carry pre-rendered countdown strings into the native pane, and the published
  snapshot carries seconds.
- **The oracle is rewritten against its recording.** `tests/bundled_info_panel.rs`
  loses the `info_tree` side ADR-42 predicted it would, and the eleven checked-in
  recordings become the expectation. **They are not regenerated**: the snapshot files
  are byte-identical after this change, which is what carries the native pane's tree
  across its own deletion.
- **The empty state is decided rather than discovered.** §14 found that the two panes
  disagree with no active session: the native one returns before painting a border,
  so the seat is a borderless gap, while a plugin pane always draws its frame. This
  change **accepts the plugin's behaviour** and pins it — a bordered Info column
  showing System (host CPU, RAM, thurbox's data-dir size) and any upcoming
  automations, which is information the pane has and a gap is not.
- **The teardown gate re-verdicts the row and moves its examples.** `info-panel-plugin`
  becomes the first `ready` pane row. Two gate tests used the info panel as their
  worked example of a *blocked* row; both now name the tasks pane, which is still
  native — otherwise the gate's own illustration would assert the opposite of the
  tree.

## Non-goals

- **The code review is not handed over.** It was the other pane named for this
  change and it is refused, with `tests/code_review_pane_handover_gap.rs` as the
  evidence: ten of its eleven rows are still blocked, including two seats (its
  changed-files list wants `RegionId::FileViewer`, which no slot names), a keyboard
  that is a capture rather than actions, and five operations no capability performs
  (review writes, `git` retargeting, clipboard/agent export, cursor writes, a
  resolved width). §20 records it as the furthest pane from handover, not the
  closest. Nothing in this change moves it.
- **No new capability, no new node, no new host binding.** The pane draws with what
  it already had; if the handover had needed a widening, that would have meant the
  reproduction was never equal.
- **The pane is not shown by default.** `default_visible` stays `false`, which is
  what `App::show_info_panel` initialised to. A handover changes *who draws a pane*,
  not *whether it is on screen* — see the design note, since the brief for this work
  asked for the opposite.
- **Focus is untouched.** This pane declares no `input`, so it needs none; the focus
  wall §21/§22 name still blocks the five panes that do.
- **`--no-default-features` loses the pane, deliberately.** That build has no plugin
  host, so after this change it has no info panel. It does not get an empty column:
  the seat is carved only by a claim, no claim can exist without the host, and
  `ToggleInfoPanel` says so. Stage B made `plugins` a default feature precisely so
  that no install is in this position.

## Impact

- Affected specs: `migration/handover` (new capability, five ADDED requirements),
  `migration/phase-0` (two MODIFIED), `migration/phase-4` (one MODIFIED),
  `migration/teardown` (one MODIFIED), `plugin-host/pane-visibility` (two MODIFIED),
  `layout/slots` (one MODIFIED).
- Affected code: `src/ui/info_panel.rs` (**deleted**), `src/ui/mod.rs`,
  `src/app/view.rs`, `src/app/mod.rs`, `src/app/key_handlers.rs`,
  `src/app/metrics_state.rs`, `src/app/acceptance.rs`,
  `src/plugin/bundled/info-panel/plugin.toml`, `tests/bundled_info_panel.rs`,
  `tests/bundled_manifests.rs`, `tests/teardown_gate.rs`.
- Docs: `docs/ARCHITECTURE.md` (ADR-50), `docs/PHASE4-PANE-READINESS.md` §25,
  `docs/PHASE6-TEARDOWN-READINESS.md`, `CLAUDE.md`.
- No schema change, no config change, no new dependency. `settings.toml`'s
  `[features] info_panel` keeps its meaning and its name; it now gates a pane the
  manifest binds it to.
