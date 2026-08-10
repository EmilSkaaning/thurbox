# Design — every declared pane is reachable, and a hidden one costs no render

Two independent mechanisms, both inside §5 of `docs/PHASE4-PANE-READINESS.md`:
one gives the keyboard reach over N panes, the other stops a hidden pane paying
for a render. They are in one change because they are the same sentence in the
audit and because the second is what makes N panes affordable.

## Part A — one action reaches N panes

### The decision: a picker, opened only when there is something to choose

`Action::TogglePluginPane` keeps its meaning ("decide which plugin panes are on
screen") and gains a second behaviour when there is more than one pane:

| declared panes | `F10` |
|---|---|
| 0 | nothing, no error |
| 1 | toggles that pane |
| ≥2 | opens `Modal::PluginPanes` |

The count-based rule is not novel here: thurbox already skips the new-session
**host picker** when no host is configured, for the same reason — a picker over
one option is a keystroke that asks a question with one answer. It also keeps the
existing spec scenario ("the action toggles the pane") literally true for the
single-pane case, which is what a stable install with one plugin looks like.

### Rejected alternatives

- **One generated action per pane** (`<plugin>.<pane>.toggle` as a *chord*,
  ADR-V21's shape). Rejected: `session::Action` is a fixed enum and
  `keybindings.json` maps chords onto its variants; `Action::rebindable_in_order`
  indexes the F1 editor's rows. Generating variants per discovered pane makes the
  keybinding namespace, the help panel's indices, and the persisted JSON depend on
  what happens to be installed — a plugin that fails to compile would silently
  drop a user's binding. The generated *commands* already exist for the headless
  case (`src/cli/commands.rs`), which is where a name-addressed surface belongs.
- **`F10` cycles through panes** (show the next, hide the rest). Rejected: it
  conflates "which pane" with "how many panes", and two panes side by side is a
  configuration the layout explicitly supports
  (`two_visible_plugin_panes_get_two_regions` in `src/ui/layout.rs`).
- **A TUI command palette over the whole command registry.** Rejected as scope,
  not as an idea: it needs its own surface (search, argument prompting, agent
  policy) and would answer this question only incidentally.
- **`F10` always opens the picker.** Rejected: it regresses the one-pane case
  from one keystroke to three, and contradicts the requirement's existing
  scenario.

### Where the types live

`ui` may not reference `crate::plugin` (`tests/architecture_rules.rs`: `ui` is
allowed `session`, `app`, `fuzzy`, `paths`). So the modal carries **plain rows**
owned by `app::modals` — `PluginPaneRow { plugin, id, title, visible }` — built
by `app` from `self.plugin_panes` under `#[cfg(feature = "plugins")]`. The
renderer `ui::plugin_panes_modal` reads only `crate::app::modals`, which is the
`ui → app` edge the TEA `view(model)` rule already allows. **No allowlist change,
and none is wanted**: a modal that borrowed `crate::plugin::PluginPane` would put
the plugin host in `ui`'s type graph for the sake of three fields.

The `Modal::PluginPanes` **variant** is cfg-gated; its rows and its renderer are
not. The intent was to gate nothing — the rows are plain data and the renderer
compiles either way — but rustc reports a variant no code constructs as dead
code, and `-D warnings` is a hard gate, so a stable build cannot carry an
unconstructible variant. The gate therefore lands on the variant and the four
`match` arms that name it (`list_selection`, the key router,
`modal_opener_pressed`, the view), while `PluginPaneRow`/`PluginPanesModal` and
`ui::plugin_panes_modal` stay ungated and keep their own tests running in both
configurations. Recorded because the reverse was written here first: the
constraint decided it, not a preference.

### Keys

`j`/`k` + `Up`/`Down` select. `Space` toggles the selected pane and keeps the
picker open (the "turn two panes on" gesture). `Enter` toggles and closes. `Esc`
closes. `F10` closes, via the existing `modal_opener_pressed` rule that already
serves the theme picker and Settings. A row click replays `Space` through
`Modal::list_selection`, matching the repo picker's choice: `Enter` there would
confirm the whole modal on a misclick, and here a misclick would hide a pane and
dismiss the window that showed what happened.

### One write path

Both routes call one method, `App::set_plugin_pane_visible(index, visible)`,
which flips the pane and writes `Database::set_plugin_pane_visible` — the same
row `thurbox-cli command run <plugin>.<pane>.hide` writes
(`plugin_pane_visible.<plugin>.<pane>` in `metadata`). Nothing about
persistence is duplicated, so the spec's "indistinguishable afterwards" is
structural rather than tested-by-coincidence.

## Part B — a hidden pane is not rendered

### The mechanism: publish the hidden set, the way kernel state is published

`PluginHost` lives on the plugin-render worker; visibility is `App` state on the
UI thread. The host cannot read the DB (`plugin` is allowed only `session` and
`paths`, deliberately — SECURITY §3), so it must be *told*.

`session::pane_visibility` is a process-wide slot holding the panes the kernel is
hiding: `publish_hidden(Vec<HiddenPane>)`, `is_hidden(plugin, pane)`. `app`
publishes it on the tick behind a change gate; `PluginHost::render_all_panes_collected`
skips a pane it reports hidden.

This is the same shape as `session::pane_context` (ADR-27) and
`session::spawn_contribution`, for the same reasons: no reference held in either
direction, no plugin code on the UI thread, no new module edge, and both sides do
bounded work (the writer replaces a small `Vec`, the reader does two string
compares per pane).

**Hidden set rather than visible set.** The reader's question is "may I skip
this?", and the safe answer for an unknown pane is no. Publishing the hidden ones
makes "unknown ⇒ visible" the structure rather than a rule someone has to
remember, keeps the payload empty in the overwhelmingly common case (nothing
hidden… or in today's default install, *everything* hidden — two entries), and
means a `thurbox-cli` process that never publishes renders exactly as before.

### Rejected alternatives

- **Send visibility down the existing UI→worker key channel.** Rejected: the
  channel is a request/reply for keystrokes served *between* render cycles, so a
  visibility message would either be read at the wrong point in the loop or need
  its own queue; and the whole path would live in `src/main.rs`, which no test
  drives.
- **Filter in `PluginHost::panes()`.** Rejected: the UI needs the full pane list
  — the picker lists hidden panes, and `set_plugin_panes` applies stored
  visibility to panes it must first know about. Filtering the pane list would
  make a hidden pane vanish from the very screen that turns it back on.
- **Let the host read the stored choice itself.** Rejected: it needs `storage`
  from `plugin`, which is exactly the edge the plugin host is built to avoid, and
  it would put a SQLite read in the render loop of every pane.
- **An `Arc<RwLock<…>>` threaded from `main` into both.** Rejected: functionally
  equivalent to the published slot but adds a parameter to `PluginHost`'s
  constructor and to `App`, and makes the two halves reachable only from
  `main.rs` — the process-wide slot is testable in-process, which is how the
  scenarios above are checked at all.

### Making the skip falsifiable

A filter applied before the call is invisible in the returned vector: "rendered
then dropped" and "never rendered" produce the same list. So `PluginHost` counts
VM renders (`render_calls`, an `AtomicU64` bumped in `render_pane`) and the test
asserts the count, not the list. Counting inside `render_pane` also means the
count includes the focused-pane render path, so it cannot be satisfied by
skipping in one caller and not another.

`pane_visibility_publishes` joins `PerfCounters` next to
`pane_context_publishes`, which is how "an unchanged set is not republished" is
checked without wall-clock timing — the same discipline the motion work used.

### The cost this accepts

A hidden pane's tree goes stale while it is hidden, so unhiding one shows its
last tree (or `Loading…`, if it never rendered) for up to one worker cycle
(~1 s). That is the staleness §7 of the readiness audit already records for the
visible case, now also paid once at unhide. The alternative — rendering hidden
panes so they are warm — is the cost this change exists to remove, and it is paid
every second forever against a wait paid once on a keystroke.

### Ownership summary, against the allowlist

| new/changed | module | edge used | allowlist |
|---|---|---|---|
| `PaneVisibility`/`HiddenPane`, published slot | `session` | none | unchanged (`session` → nothing) |
| render skip, `render_calls` | `plugin` | `session` | unchanged (`plugin` → `session`, `paths`) |
| `PluginPanesModal`, publisher, setter | `app` | everything | unchanged (coordinator) |
| picker renderer | `ui` | `app` | unchanged (`ui` → `session`, `app`, …) |

`tests/architecture_rules.rs` is expected to be **byte-identical** after this
change; if it needs an edit, the design above is wrong.
