# The automations pane and its plugin converge on one frame and one fit

## Why

The automations pane is the closest of the four remaining native panes to a
handover: its rows are reproduced exactly, five of its keys ship, and every host
decision it needed — the seat (ADR-46), the focused border (ADR-51), the render
trigger (ADR-49) — is closed. `tests/automations_pane_handover_gap.rs` records what
is left, and exactly one of those rows is about **drawing**: `no-fitted-name`.

Two things would change on screen if the pane were handed over today, and neither is
in the reproduction's fault:

- **The name's fit.** `resolve_rows` cuts the name to
  `width − marker − summary` with `ui::truncate_ellipsis`. The plugin cannot: a pane
  is never told a width. ADR-52 closed the *vocabulary* — a run may declare that it
  yields its width and the kernel ellipsizes the group — and the tasks pane adopted
  it. This pane did not, so its tree carries an already-cut name while the plugin's
  carries a whole one, and **no width makes the two equal**.
- **The frame.** Every other pane in thurbox draws `ui::focus_block`: rounded
  borders, a focus-styled title, `accent_bright` when focused. This pane builds its
  own `Block` with square corners, an unstyled title and `border_focused`, and it
  draws its `Active` level exactly as `Inactive`. `App::paint_plugin_pane` draws
  `focus_block` — as it must, since the seat decides *where* a pane is drawn and
  never *how* — so a handover would silently change the band's corners, its focused
  colour and its title styling. A handover whose claim is that a user notices nothing
  cannot also be the change that restyles a pane's border.

Both are convergences of the **native** pane onto what a seated pane can be, so they
belong before the handover rather than inside it — the ordering ADR-52 established
when `ui::tasks_panel` stopped fitting in the same change that let a run yield.

## What Changes

- **The native pane stops fitting its name.** `resolve_rows` loses its `width`
  argument and publishes the name whole; `row_node` marks every run of the name
  `ellipsize`, so the kernel cuts the group with the same `ui::truncate_ellipsis` and
  leaves the marker and the whole summary tail their columns. The name is split at
  its search-match offsets, so this is the first pane to rely on ADR-52's
  "consecutive yielding runs share one budget" rule for more than one run in
  practice.
- **The plugin declares the same fit.** `src/plugin/bundled/automations/init.luau`
  builds the name's runs through the style-table form with `ellipsize = true`. The
  marker and the summary keep their intrinsic widths, which is what makes the
  summary survive a long name — the failure a clip at the pane edge produced.
- **The native pane draws the shared frame.** `render_automations_pane` builds
  `ui::focus_block(" Automations ", focus)` instead of its own `Block`. Three visible
  consequences, stated rather than absorbed: rounded corners (matching the ` Sessions `
  block directly above it), `accent_bright` rather than `accent` while focused, and an
  **accent border at `Active`** — the level the pane already receives while the
  central-pane automation editor or its run history holds the keyboard, and which it
  has been drawing as unfocused.
- **The retained pre-port oracle converges with it.** `legacy_render` in that
  module's tests builds the same block, so the cell-for-cell comparison against the
  span renderer keeps being a statement about the pane's *rows*.
- **The thirteen recordings are regenerated from the native builder.** ADR-42 requires
  this and permits it only while that builder exists: the recording is the pane's tree,
  so a change to the pane's tree is a change to the recording. The diff is verified as
  a multiset — every moved line is the same line plus `ellipsize`.
- **The oracle's enumerated divergence is replaced by its opposite.**
  `a_name_wider_than_the_column_is_fitted_by_the_kernel_only` becomes an assertion
  that at a narrow width the two panes paint the *same* frame, ellipsis and summary
  tail included.
- **`no-fitted-name` closes**, and the handover gap gate then records no drawing row
  outstanding at all — which is the state the tasks gate reached before its handover.

## Non-goals

- **No handover.** `src/ui/automations_panel.rs` stays, `src/app/view.rs` keeps
  drawing it, the bundled pane stays hidden, and the teardown gate's
  `automations-plugin` row stays blocked. What this change removes is the last reason
  the two panes could not be *identical*.
- **No new vocabulary and no new capability.** `ellipsize` exists; `focus_block`
  exists. If either had to be invented here, the reproduction was never equal.
- **The empty-state line stays as it is.** It reads `none — Ctrl+N to add` while
  focused, naming a chord that is rebindable — the same defect the tasks pane's hint
  row had, and the tasks handover resolved by making the row kernel *seat chrome*.
  This line is inside the pane's rows rather than beside them, both panes draw it from
  the published `focused` flag, and moving it is the handover's decision to take with
  the rest of that pane's chrome.
- **The width the kernel fits at is not published.** ADR-52's rule stands: a
  declaration crosses, a geometry does not.

## Impact

- Affected specs: `migration/phase-4` (one MODIFIED), `migration/handover` (one
  ADDED).
- Affected code: `src/ui/automations_panel.rs` (the width came from `inner.width`
  inside the pane, so no caller changes), `src/plugin/bundled/automations/init.luau`,
  `tests/bundled_automations_panel.rs`,
  `tests/automations_pane_handover_gap.rs`, `tests/snapshots/bundled_automations_panel__*`
  (regenerated), `src/app/snapshots/thurbox__app__acceptance__empty_welcome_screen_renders.snap`
  (the band's corners).
- Docs: `docs/ARCHITECTURE.md` (ADR-55), `docs/PHASE4-PANE-READINESS.md` §30.
- No schema change, no new dependency, no settings change.
