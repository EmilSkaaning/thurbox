# Every declared pane is reachable, and a hidden one costs no render

## Why

Phase 4 shipped its first bundled pane (`src/plugin/bundled/info-panel/`) and
left the half-open gap `docs/PHASE4-PANE-READINESS.md` §5 records. Two facts
about the tree as it stands:

1. **`F10` can only reach the first declared pane.** `App::toggle_plugin_pane`
   (`src/app/mod.rs`) mutates `self.plugin_panes.first_mut()`. Two bundled
   plugins declare a pane today — `hello` and `info-panel` — so the pane the
   previous change shipped is **unreachable from the keyboard entirely**: the
   only ways to put it on screen are `thurbox-cli command run
   info-panel.info.show` and editing the stored choice by hand. A pane a user
   cannot show is not a pane.
2. **A hidden pane still enters its VM.** `PluginHost::render_all_panes_collected`
   (`src/plugin/lifecycle.rs`) renders every pane *declared by a running
   plugin*, and `App::render_plugin_panes` (`src/app/view.rs`) then throws away
   the trees of the ones that are hidden. The existing requirement "A hidden pane
   costs nothing to draw" is honoured for layout and **not** for the render:
   today's default install pays a Luau render per second for two panes nobody is
   looking at, and Phase 4 schedules seven.

This is not v1 behaviour being replaced — v1 had no plugin panes at all. It is
v2's own `plugin-host/pane-visibility` contract being finished: the requirement
that the toggle is a rebindable action was written when one pane was the only
case, and the requirement that a hidden pane costs nothing to draw was only ever
enforced against the layout. Both are behind the same feature gate as everything
else in the plugin host: `#[cfg(feature = "plugins")]` (the `plugins` Cargo
feature).

Closing this gap was chosen over porting a second pane because the second pane
would land in the same hole: reproduced, asserted equal, and unreachable. The
gap is also the one the previous round named as open and deliberately deferred —
"a keybinding decision with its own surface".

## What Changes

- **One action, two behaviours, decided by how much there is to choose.**
  `Action::TogglePluginPane` (`F10`, rebindable, unchanged as an action) keeps
  toggling directly when exactly one pane is declared, and opens a **pane
  picker** when two or more are. This mirrors thurbox's existing rule for the
  new-session host picker, which is skipped when there is nothing to choose.
- **A new kernel-owned modal**, `Modal::PluginPanes`: one row per declared pane,
  plugin-qualified, with a checkbox showing whether it is on screen. `j`/`k`
  select, `Space` toggles the selected pane and stays, `Enter` toggles and
  closes, `Esc` closes, `F10` closes (the opener-toggles-closed rule the theme
  picker and Settings already use). A row click toggles that row, through the
  existing modal row-hitbox path.
- **Toggling from the picker persists exactly what the direct toggle
  persists** — the same `plugin_pane_visible.<plugin>.<pane>` metadata row the
  generated `<plugin>.<pane>.toggle` command writes. There is one write path.
- **The render worker skips a pane the kernel is hiding.** A new published slot,
  `session::pane_visibility`, carries the *hidden* set; `app` publishes it on the
  tick behind a change gate, and `plugin` consults it before entering a VM.
  Unknown means visible, so a host with no publisher (a `thurbox-cli` invocation)
  behaves exactly as it does today.
- **The skip is observable**: `PluginHost::render_calls` counts VM renders, so
  "a hidden pane costs no render" is a test rather than a claim, and
  `pane_visibility_publishes` joins the perf counters so the publication cannot
  quietly become per-tick work.

## Capabilities

- `plugin-host/pane-visibility` — MODIFIED: the toggle requirement now covers N
  panes. ADDED: a hidden pane is not rendered.
- `migration/phase-4` — ADDED: a ported pane must be reachable by the keyboard,
  so the next port cannot repeat this.

## Non-goals

- **Per-pane keybindings.** No `<plugin>.<pane>.toggle` *chord* is generated.
  `Action` is a fixed enum that `keybindings.json` maps chords onto; generating
  one action per discovered pane would make the keybinding space depend on what
  is installed, and the F1 editor's stable indices with it. The picker gives N
  panes one key without that.
- **A general command palette.** The generated visibility commands already exist
  and run headlessly; surfacing the whole command registry in the TUI is its own
  change with its own surface.
- **Focus.** Which pane takes `Ctrl+L` focus is unchanged; this change is about
  which panes are on screen.
- **Porting another pane.** None is ported here.
- **Event-driven plugin render.** The ~1 s staleness recorded in §7 of the
  readiness audit is untouched; skipping hidden panes makes the worker cheaper,
  not more prompt.

## Impact

- `src/app/modals.rs` — `PluginPanesModal`, `PluginPaneRow`, the `Modal` variant
  and its `list_selection` arm.
- `src/app/key_handlers.rs` — modal routing, the picker's key handler, the
  opener-closes rule, and the `TogglePluginPane` dispatch.
- `src/app/mod.rs` — `toggle_plugin_pane` split into "one pane" and "open the
  picker", the shared per-pane setter, and the visibility publisher.
- `src/app/view.rs` — one render arm.
- `src/ui/plugin_panes_modal.rs` — new renderer. Reads only `app::modals` data,
  never `crate::plugin`, so the `ui` allowlist is unchanged.
- `src/session/pane_visibility.rs` — new published slot (pure data).
- `src/plugin/lifecycle.rs` — the render skip and the render counter.
- `src/app/metrics_state.rs` — one perf counter.
- Docs: `CLAUDE.md` (the `F10` row), `docs/ARCHITECTURE.md` (ADR-28),
  `docs/PHASE4-PANE-READINESS.md` (§5 closed), `docs/PERFORMANCE.md` (the
  counter).
- `tests/architecture_rules.rs` — unchanged, and that is a claim this change
  must keep: no new module edge.
- `tests/teardown_gate.rs` — unchanged. No pane is handed over here, so no
  replacement row flips.
