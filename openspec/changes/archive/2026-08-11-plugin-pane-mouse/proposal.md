# Make a plugin pane clickable, one row at a time

## Why

Every native pane in thurbox is clickable, and by one mechanism: a renderer
returns `ui::RowHitbox`es, `App::view` records them as `ClickAction`s in
`App::click_targets`, and `handle_mouse_click` hit-tests them — `SelectSession`,
`SelectTask`, `SelectAutomation`, `SelectFileRow`, `ReviewRow`, each followed by
the pane's whole-rect `FocusPane` fallback. Clicking a row selects it *and*
focuses the pane that owns it, and hovering underlines what a click would hit.

A plugin pane has none of that. It records **no click target at all**: clicking
one does not focus it, does not reach the plugin, and does not even highlight
under the pointer — the mouse is the one input channel where a plugin pane is not
merely thinner than a native pane but absent. `plugin-host/input` said "no mouse"
as a non-goal, and this is the change that closes it.

It matters for a replacement rather than as a nicety: a pane replacement must
"behave exactly like existing panes", and `docs/PHASE4-PANE-READINESS.md` lists a
click among the things every ported pane left in the kernel (the session-list port
records "keys and hitboxes: no `j`/`k`, no `Shift+J` reordering, **no click**").

A second, smaller thing surfaces with it. `InputFocus::PluginPane` names *a*
plugin pane, not *which*: `App::focusable_plugin_pane` returns the first focusable
one, so with two focusable panes on screen the second can never receive a key.
Clicking is what makes that visible — a click names a pane by construction — so
this change also makes focus remember which pane it landed on.

## What Changes

- **A plugin pane's rows get hitboxes.** The pane renderer reports one rect per
  row of the pane's **outermost list**, carrying that row's 1-based index — the
  same numbering `ui.list`'s `selectedRow` uses, so a plugin's cursor and a click
  speak about rows the same way.
- **A click is recorded like any other row click.** A new `ClickAction` carries
  the pane's identity and the row; `App::view` records it before the pane's
  whole-rect focus fallback, so an on-row click wins and a click on the pane's
  empty space still focuses it. Hover highlighting follows for free, because it
  runs off the same recorded targets.
- **A click reaches the plugin as an event on the existing bounded channel**, the
  one a key already uses: the plugin is told the pane and the row, answers whether
  it consumed the click, and the UI thread waits at most the same 50 ms.
- **Focus follows the click.** Clicking a focusable plugin pane focuses *that*
  pane, and the keys that follow go to it. Clicking a pane whose plugin never
  declared `input` focuses nothing and delivers nothing, exactly as focus
  navigation already skips it.
- **No coordinates cross.** A plugin learns which row was clicked, never where —
  no x, no y, no rect, no width. The model has refused a plugin its geometry four
  times (ADR-26, ADR-29, ADR-30, ADR-31) and a click is not the reason to stop.

## Capabilities

### New Capabilities

- `plugin-host/mouse`: what a click resolves to, which rows are clickable, what
  the plugin is told, what happens to a click it does not consume, and what is
  deliberately not reported.

### Modified Capabilities

- `plugin-host/input`: focus on a plugin pane names *which* pane, so keys and
  clicks reach the pane the user pointed at rather than the first one declared.

## Non-goals

- **No pane is ported.** No bundled plugin declares `input`, so none of them is
  clickable yet; the native panes stay on screen, every insta snapshot stays
  byte-identical and `tests/teardown_gate.rs` is untouched.
- **No drag, no wheel, no hover event.** A wheel tick over a plugin pane keeps
  doing what it does today (nothing), text selection is unchanged, and the plugin
  is not told about the pointer moving. One event, the one native panes act on.
- **No coordinates, ever.** See above.
- **No right-click or modifier semantics.** A click is a click; a plugin that
  wants a second gesture declares a keybinding.
- **No scrollbar.** A plugin pane's list still has no track to drag;
  `docs/PHASE4-PANE-READINESS.md` §9 records that as its own gap and it stays
  open.
- **No new capability.** `input` already gates receiving input, and a click is
  input. A plugin that did not ask for it is not handed a click, for the reason it
  is not handed a key.

## Impact

Behind the existing `plugins` Cargo feature. A default build records no plugin
click target because it has no plugin pane.

`src/ui/plugin_pane.rs` (row hitboxes from the outermost list),
`src/app/mod.rs` (`ClickAction`, the focused-pane identity, delivery),
`src/app/view.rs` (record them), `src/app/key_handlers.rs` (dispatch),
`src/plugin/runtime.rs` + `src/plugin/lifecycle.rs` (`onClick`), `src/main.rs`
(serve the event), `src/plugin/bundled/thurbox.d.luau`,
`docs/ARCHITECTURE.md` (ADR-36), `docs/PHASE4-PANE-READINESS.md`, `CLAUDE.md`.
