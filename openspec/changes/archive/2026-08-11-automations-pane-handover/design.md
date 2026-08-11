# Design

## 1. The route, and why the shipped port has to give something up

Two routes exist for a pane with keys (ADR-51), and a manifest refuses both in one
pane:

- **the plugin's own keys** — `input` plus pane-addressed bindings, acting through
  granted capabilities. This is what the bundled `automations` plugin *is*: five
  bindings, `automations-write`, its own cursor across renders.
- **the kernel's keyboard** — `key_context = "Automations"` means *I am thurbox's
  automations pane*; the kernel resolves that context's actions and performs them
  itself while the pane holds focus.

The first route reaches five of seven keys and can never reach the other two: `n`
creates a record (the write seam has no creation operation, by construction — ADR-35:
creation has no id to address, so a grant to create is a grant to add rows without
bound) and `Enter`/`e` opens a **central-pane** editor, which is text authoring
`automations-write` is defined to exclude, into a focus a plugin cannot take. It also
loses the editor and the run history entirely, because
`App::render_central_pane` branches on three *native* focuses and a plugin-keys pane
is `InputFocus::PluginPane`.

So the handover takes the second route, and the port loses its keys. That is a
**reduction** in what an installed plugin may do — four capabilities to two — and it
is worth stating as the finding rather than as a cost: the pane that looked like it
needed the widest grants needs the fewest. Its five ported keys were a demonstration
that a plugin pane *can* act; they were never how this pane should be driven.

### Rejected: keep `input` and add the two missing operations

A creation operation plus a field-writing operation would let the plugin answer all
seven keys on its own route. Refused twice over: ADR-35 refuses creation on its merits
(unbounded rows), `automations-write` is *defined* to exclude text authoring, and even
with both the pane would still lose the central editor and the run history, because
those follow a focus and not a grant. Two new grants to reach a strictly worse
outcome.

### Rejected: teach `render_central_pane` about a plugin pane

The other way to close the central-seat row is to make the branch fire for
`InputFocus::PluginPane` when the focused pane declares itself the automations pane.
That is the same fact ADR-51 already encodes — and encodes better, by reusing
`InputFocus::Automations` so `focus_key_context`, the ring, the editor's return paths
and `Esc` need no arm at all. A second mechanism for one fact is how a handed-over
pane comes to *almost* work.

## 2. Where `row_summary` goes

`ui::automations_panel::row_summary` has two consumers: this pane, and the `Ctrl+P`
list modal through `app::automation::format_automation_summary`. After the deletion it
has one, plus the bundled plugin's copy which the oracle compares against it.

**Chosen:** `src/ui/automations_list_modal.rs` — the module that renders the surface
still composing it.

Rejected alternatives:

- **`src/ui/mod.rs`, beside `format_countdown`.** Defensible: that function lives
  there for the same reason (two surfaces must not disagree) and `row_summary` calls
  it. Refused because `ui/mod.rs` is the layer's shared vocabulary and this is one
  surface's row format; the modal's module is where a reader looking for the modal's
  row goes.
- **`src/app/automation.rs`, beside `format_automation_summary`.** This is the
  info panel's precedent (ADR-50 moved `SystemMetrics` to its value's owner). Refused
  because the thing being moved is *display text composition*, and `app` is the
  coordinator: it would split one rule across two layers, with the countdown formatter
  it depends on staying in `ui`. It would also need a `pub` item in `app` for the
  oracle to reach, where `ui` is already public for exactly this.
- **`src/session/`.** It is presentation, and `session` is data.

## 3. `default_visible = true`, and the rule it generalises

Every previous bundled pane seeds hidden, and `tests/bundled_manifests.rs` binds that
as a rule with one exemption list for handed-over panes. Both entries so far
(`info-panel`, `tasks`) replaced a pane whose kernel flag initialised to `false`, and
the rule's second half asserts a handed-over pane *still* seeds hidden **and** names a
toggle action.

This pane breaks both halves for one reason: the native band was **always on screen**
and had no toggle action at all. The rule generalises exactly as its own doc predicted
— "a later handover of a pane that *did* default to visible (none does today) would
want the opposite value for the same reason". So the exemption list carries the native
default per entry and the rule becomes:

- the pane seeds at its native counterpart's default, and
- a pane that seeds **hidden** must bind an action, because otherwise nothing could
  reveal it. A pane that seeds visible need not.

That is stronger than "seeds hidden", not weaker: it now catches a handed-over pane
that seeds *hidden* when its native pane was visible, which the old rule silently
permitted.

## 4. The pop-in, which is the spike's prediction arriving

`docs/SPIKE-SESSION-LIST.md` §"Bar 2" recorded a structural point Phase 4 would owe:
the host starts detached and the first frame does not wait for it, so "a plugin
session list therefore either pops in a moment after the first frame, which is a
visible regression of a different kind, or the first frame has to block on a VM".

The info panel and the tasks pane both seed hidden, so neither exhibited it. This one
does: on launch the left column is the session list alone, and when the host publishes
its panes the band appears beneath it.

Three options were considered.

1. **Carve the band on the feature flag, as today** (`features.automations || seat`).
   No reflow — but a *blank* band until the pane arrives, and a permanently blank one
   whenever the pane is absent for any other reason (a plugin that failed to compile,
   a user-shadowing `automations` plugin with no pane, a build with no host). That is
   the empty-column failure the teardown gate exists to prevent, and ADR-50 already
   ruled on it: "a retained flag would carve a column nothing paints".
2. **Block the first frame until the host publishes.** Refused by
   `plugin-host/panes`: the kernel never calls a plugin during a frame, and the whole
   startup design is that a slow or wedged plugin cannot delay the interface.
3. **Accept the reflow.** Chosen. It is one layout change, it is self-correcting, and
   it degrades honestly in every case option 1 handles badly — no pane, no band, and
   the session list gets the column.

The residual cost is real and is named rather than buried: a launch has a visible
column split. Reducing it is a *startup* problem (how soon the host publishes), not a
pane problem, and it is the same problem for every pane that will follow.

## 5. The wrap: decided, and why it needs no owner

The left column is one circular list — `j` past the last session drops into the
automations pane, `k` at the first automation returns to the last session, and both
ends wrap. With both panes becoming plugins, "where does the wrap live" looks like a
new question.

It is not. The wrap is four lines in two kernel handlers
(`act_session_list_next`/`prev`, `automations_pane_move_down`/`up`) that move
`self.focus` between `InputFocus::SessionList` and `InputFocus::Automations`. On
ADR-51's route a handed-over pane is focused *as* the kernel's pane of that name, so
both ends of the wrap are kernel focuses **whoever draws either pane**. The wrap
therefore survives one handover, both handovers, or neither, with no owner to assign.

What does change is its **gate**. Today it reads `self.features.automations`, which
answers "is the feature on" and was a good enough proxy while the kernel drew the pane
unconditionally. It becomes "a pane provides the automations list"
(`pane_keyboard_taken(KeyContext::Automations)`), which is the question the wrap
actually asks: without it, `j` at the last session would drop focus into a pane that
is not on screen — the failure mode `show_tasks_panel`'s three hidden jobs were about
(ADR-53).

This is also why the plugin's half of the wrap is *deleted* rather than kept. The port
had the plugin decline the key at either edge, honestly, as its half of a wrap it
could not complete. On this route there is no plugin half: the kernel never asks it.

## 6. What the oracle keeps

The tasks handover dropped both differential edges and kept its recordings. This one
keeps a third thing, and the distinction is worth stating: **not every edge in an
oracle is differential against the deleted builder.**

- `automations_tree` / `resolve_rows` equality — differential, dropped. The
  recordings are the expectation and are *not* regenerated; byte-identical after the
  deletion is the payoff (ADR-42).
- The five key tests — they measured the plugin's own keys against the database.
  Those keys are gone, so the tests go with them. Their claims are preserved in
  ADR-56, because they are still true of any pane that takes the other route.
- `the_plugin_composes_the_summary_thurbox_composes` — **kept**. It compares the
  plugin's composed summary against `row_summary`, which survives the deletion because
  the `Ctrl+P` modal composes it too. So it is not differential, and it is the only
  test that holds the plugin to a *rule* rather than to a recording. 192 combinations
  of schedule × action × enabled × countdown, which no snapshot set covers.

## 7. What `--no-default-features` loses, precisely

The pane is the only door to `InputFocus::Automations`, so that build loses the band,
the central automation editor, its run history and the `Enter`-opens-that-run's-session
jump. It does **not** lose automations: `thurbox-cli automation` is untouched, the TUI
still fires due schedules on its tick, the heartbeat keeper still fires them headless,
and `Ctrl+P` still opens the list modal and its overlay editor — which is a complete
authoring surface reached through a `Modal`, not a pane.

That is a smaller loss than the tasks pane's (which lost its whole TUI surface,
because its preview, editor and picker all hang off the pane's focus), and it is
stated rather than left to be discovered. The monkey invariant gets the same
strengthening: `focus == Automations | AutomationEditor | AutomationRunHistory` now
implies a pane provides the automations list, which is unsatisfiable without the plugin
host — so that build can never reach the focus.
