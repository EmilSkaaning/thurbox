# Design

## Why two gates and not one table

`tests/teardown_gate.rs` already answers "may this renderer be deleted", and its
answer for both panes is *no* for ADR-37's shared reason. That is a different
question from "what would a handover need", and one table answering both produces
failures that do not say which question moved. The tasks pane and the file viewer
each got their own gate for this reason; these two follow.

Two files rather than one shared "left column" gate, because the two panes fail
differently and a merged table would blur it: the automations pane's keys are
five-sevenths ported and its wall is its **seat**; the session list's seat is
equally blocked but its wall is its **keys**, and specifically that its cursor is
the active session.

## The third gap kind

The earlier gates classify a row as *structural* (a power withheld on purpose) or
*vocabulary* (something the catalogue cannot say). Two requirements here are
neither:

- **the render trigger** — the worker re-renders every pane on a fixed 1 s
  interval (`PLUGIN_RENDER_SLICE × PLUGIN_RENDER_SLICES` in `src/main.rs`) and
  nothing tells it kernel state moved;
- **a pane's knowledge of its own focus** — every published `focused` field
  describes the *native* surface being reproduced, so a plugin pane cannot learn
  that focus left it.

Both are closable with no new plugin-facing concept and no node: they are host
wiring plus a rate policy. Filing them as structural would claim the model
forbids them, which it does not; filing them as vocabulary would say the drawing
catalogue is short a word, which it is not. So the gates carry `Gap::Wiring`, and
the derived verdict treats all three kinds as blocking while the *ordering* of the
work follows from the kind — wiring first, since it is the cheapest and the
session list's staleness stops being cosmetic the moment the pane is the one a
user navigates with.

## What the probes read

Every probe reads source text, the way a human auditor would, so both gates run
and mean the same thing with or without the `plugins` feature — the property that
lets them sit beside `tests/teardown_gate.rs` rather than inside the
feature-gated oracles. Concretely:

| Row | Probe |
|---|---|
| scoped keys silenced | `App::focus_key_context`'s body maps the plugin-pane focus to `Global` |
| the six session-list actions | `Action::context`'s `KeyContext::SessionList` arm |
| no session write | `KernelWriter`'s method list mentions no session |
| the seat | `PaneSlot`'s variants |
| the render trigger | `src/main.rs`'s slice constants, and that no nudge names the snapshot |
| the central seat | `render_central_pane`'s branch on the automations focuses |
| the shared summary | `src/app/automation.rs` calls `ui::automations_panel::row_summary` |
| the module is the model | `src/app/mod.rs`'s references to `project_list::` |

A probe reading a *method body* is scoped to that body, so an unrelated mention
elsewhere in the file cannot flip a verdict — the rule `tests/teardown_gate.rs`'s
`block` helper already follows, and the reason both gates carry their own copies
of those helpers.

### Rejected: assert the walls by driving a plugin pane

A test that started the bundled session-list plugin, gave it focus and pressed
`j` would show the key doing nothing — and it would be asserting the *absence* of
an effect, which passes for any number of wrong reasons (the plugin declined the
key, the host dropped it, the fixture never had a second session). It also needs
the `plugins` feature, so the verdict would be invisible in the default build,
which is the configuration the release is cut from.

### Rejected: fold both verdicts into `tests/teardown_gate.rs` as extra rows

Its rows are the *inventory*: one per v1 capability, with a single derived
verdict. A handover requirement is not a capability and its absence does not gate
a deletion on its own (the deletion is already gated). Adding them would make one
failure mean either "a replacement landed" or "a pane's requirement moved".

### Rejected: add `sessions-write` now, scoped to reorder and sort

The brief asked for the missing write capability. It is the right *shape* — both
are single-keystroke record writes, exactly ADR-35's rule — and adding it now
would be the third capability in the host with **no consumer**, joining `input`
before ADR-41 and `tasks-write`. The reason it would have no consumer is not
timing: the keys it would enable act on the row the user is looking at, and for
this pane that row is the kernel's cursor — the active session — which a plugin
can neither move nor be told about. So the grant would widen a plugin's reach
over the database while the pane it exists for still could not use it. It is
recorded as a row, with the order it should be added in: after the cursor
question is answered, not before.

## What would actually unblock each pane

Recorded so the next attempt starts from an ordering rather than from a list:

**The automations pane** needs a seat before anything else, and its seat needs a
decision the plugin protocol has so far refused — whether plugin content may size
a kernel region (the native pane's height is `(count + 2).clamp(3, 10)`). Then the
central-pane coupling has to be broken or reproduced, which is the same
central-seat question the tasks pane's editor raised. Its keys are the least of
its problems.

**The session list** needs the cursor question answered first, and the honest
statement is that there are only two answers: either kernel view state becomes
writable by a plugin under a capability (which makes "the active session" a
plugin-writable thing — the widest grant in the host, and the one that can change
what the whole interface is showing), or the pane keeps a kernel-owned cursor and
a plugin only supplies its rows — which is the retreat `docs/SPIKE-SESSION-LIST.md`
named, and is not a plugin pane at all. That choice is ADR-V1's, not a pane
port's, so the gate states it rather than picking one.
