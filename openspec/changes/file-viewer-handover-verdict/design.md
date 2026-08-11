# Design

## The question, and why the recorded answer had become wrong

The brief for this work asked, of the file viewer: *widen the `files` capability the
minimum the pane needs, state exactly what it now grants and still refuses, and do not
grant unbounded file reads or process launch.*

The answer is **none**. Not a minimum widening — nothing.

`tests/file_viewer_pane_input_gap.rs` was written when there was one way to hand a pane
over: the plugin takes the keys. On that route the pane's seven actions need a view
write, a directory listing and a process launch, which is why ADR-39 refused it and why
the gate's rows read the way they do. ADR-51 added a second route — the pane declares
`key_context = "FileViewer"`, is focused as `InputFocus::FileViewer`, and the *kernel*
resolves those seven actions against `App::file_viewer` exactly as it does today. The
directory read stays in `FileViewerState::activate`. The editor launch stays in
`App::file_viewer_expand`. The plugin draws.

Leaving the old rows in place would be worse than deleting them: the next person to
read that file would go looking for the widest grant in the host, find it refused twice,
and conclude the pane is unhandoverable — when what it actually needs is three
decisions, none of which is a grant.

## Decisions

### The rows are re-verdicted, not deleted, and the closed ones keep half their job

`no-filesystem-read` is the important one. Its need ("filling a directory the first time
it is expanded") is met by the kernel, so as a *handover requirement* it is closed. But
the fact it was really recording — **`Capability::Files` publishes basenames and nothing
else; no binding lists a directory; the published row carries no path** — is exactly what
a later change could erode while nobody was looking, and the gate is the only thing
watching it.

So the row closes on a conjunction: the keyboard is declarable **and** the capability is
still narrow. If someone adds `readDir` to the module surface, this row fails — not
because the pane became unhandoverable, but because the claim "the handover needed no
grant" stopped being true. That is the shape `plugin-host/capabilities`'s own tests
cannot express, because they check what the catalogue *is* rather than what a decision
*depended on*.

### The sub-mode rows are re-scoped rather than closed or kept blocked

`sub-mode-keys-are-not-rebindable` says the `/` sub-mode abandons the pane's scoped
context so every character types into the query, and that its keys are matched literally
rather than resolved through an action. Still true — and true *before and after* a
handover, because the sub-mode is kernel state either way. It was a blocker only for a
plugin holding the keys.

Recorded as a **property**, then, with the consequence named: the pane's `/` keys are not
in the keybinding editor today, and a handover neither fixes nor worsens that. Same for
`no-query-write`: the query is the kernel's, and the kernel draws the bar.

### The three added rows are decisions, and the table says so

Two kinds of thing can block a handover: a power the host withholds *in principle*, and
a decision nobody has taken. The gate's `Gap` enum already distinguishes `Structural`
from `Vocabulary`; the three new rows are neither — they are unmade decisions with an
obvious shape and a known mechanism:

| Row | The decision |
|-----|--------------|
| `no-file-viewer-seat` | `PaneSlot` names no seat for this column's first occupant. Adding one is the tasks pane's own change, one line — *but* it must come with the rule below, or the review breaks. |
| `the-module-is-the-model-and-the-window` | Where `FileViewerState` goes (`app`), and where `visible_window` goes (`ui`, since every plugin list calls it). Five call sites in four modules. |
| `the-column-has-a-second-kernel-occupant` | What a claim does while a code review owns the column. |

They are tagged `Vocabulary` — the gate's name for "today's host cannot say it, and
could" — because that is what they are: mechanisms that exist for one row of chrome and
one seat, not powers the model withholds. The distinction matters for the ordering, which
is why `the_verdict_is_derived_from_the_blockers` now asserts that **nothing outstanding
is structural**. That single assertion is this change's headline: the file viewer is no
longer blocked by anything about what a plugin *is*.

### The third decision is the one with a trap in it

While a review is open, `App::layout_for` force-shows the file-viewer column and
`App::render_file_viewer` draws the review's changed-files list into it instead. That
list has its own focus (`InputFocus::ReviewFiles`) and its own keys, and ADR-45 records
it as wanting `RegionId::FileViewer` **specifically** — which is why ADR-46 refused to
give a plugin a second seat for it.

So the seat has two kernel occupants, and ADR-46's rule ("a visible plugin pane takes its
seat and the kernel's pane for it is not drawn") would hand the column to the plugin
while the review needed it. The likely rule is the opposite of the general one — the
claim yields while a review owns the column — and *that* is the decision: the first seat
where the plugin does not simply win. Taking it in the same change as a 900-line
relocation is how it gets taken carelessly.

## Rejected alternatives

- **Hand it over anyway, and accept the review losing its changed-files column.** It
  would replace a working navigation aid with an empty pane whenever both features are
  used together, which is the failure the teardown gate exists to prevent, reached from
  inside a change meant to honour it.
- **Widen `files` to close the rows honestly.** Refused twice (ADR-39, and again here):
  it is unnecessary, since the kernel keeps the keys. Widening it *because a gate row
  said so* would be closing a checkbox with the widest grant in the host.
- **Delete the gate, on the grounds that its question is obsolete.** Five of its rows
  are the only place recording that the grants were unnecessary rather than absent. The
  tasks pane's gate could be retired because its pane was handed over in the same change
  (ADR-53); this pane's cannot, because the verdict is still "no".
- **Do the relocation now as groundwork and leave the rest.** ADR-39 rejected this as
  "motion without a destination", and the objection has only half-lifted: the
  destination exists now, but a 900-line move whose only proof is that the tests still
  pass belongs in the change that *uses* it, where the oracle and the seat are what prove
  it landed correctly.
- **Rename the file to `file_viewer_pane_handover_gap.rs`** to match its new question.
  Churn: the name is referenced from four documents and the git history is the more
  useful continuity. Its module note says what it now measures.
