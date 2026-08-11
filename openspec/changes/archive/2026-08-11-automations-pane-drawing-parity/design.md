# Design

## 1. What is actually being decided

Only one thing is genuinely open: **who converges**. A reproduction and its native
pane differ in two places, and each difference could be closed from either side.

| Difference | Close it in the plugin | Close it in the kernel |
|---|---|---|
| The fitted name | publish a width, or publish an already-cut name | the native pane stops fitting and *declares* the fit (ADR-52) |
| The frame | let a seated pane bring its own block | the native pane adopts `focus_block` |

Both right-hand answers were already decided by earlier changes; this change is
where the automations pane obeys them. The rest of the document is the rejected
alternatives, because each one is a plausible shortcut that would have produced a
handover a user notices.

## 2. Rejected: publish the resolved width

The obvious fix for a fitted name is to tell the pane its column. It is refused for
the fifth time, and the reasons compound rather than repeat:

- A width is resolved **during a frame**; the snapshot is published on the **tick**.
  A pane reading last tick's width would cut its rows to the wrong column on the
  frame after a resize, which is worse than a clip because it is intermittent.
- A seated pane's rect and the *native* pane's rect are not the same thing while both
  exist, so a published width would be a lie for whichever pane is not drawn.
- It generalises badly: the pane that genuinely needs geometry is the code review
  (wrapping, side-by-side pairing), and a width does not close that either.

`ellipsize` costs one boolean and resolves in the renderer, where the width already
is.

## 3. Rejected: publish an already-fitted name

The kernel could keep fitting and publish the *cut* string, which would make the two
trees equal today with no new vocabulary. Refused because it inverts what a pane is:
the snapshot would carry the pane's rendering rather than the model's fact, and
`session::pane_context`'s whole rule is that a plugin composes text from parts the
kernel resolved only where a sandbox cannot (a cron label, a countdown). A name is
not one of those. It would also break the moment the plugin's pane and the native
pane had different widths — which is exactly the state in which the reproduction is
compared.

## 4. Rejected: leave the native pane fitting, and let the plugin clip

This is the status quo, and it is what makes the row a blocker rather than a
divergence. `tests/bundled_automations_panel.rs` currently compares at a width where
the fit is a **no-op** (`WIDE = 80`, asserted by
`the_comparison_size_adjusts_nothing`) and enumerates the narrow case as a known
difference. That is honest, but it means the equality claim says nothing about the
pane at its real size: the left column is ~24 columns at 120, where every one of this
pane's rows overflows, because the summary tail alone is around 30. So the enumerated
divergence covers *the normal case* and the equality covers a case that does not
occur.

Worse, the loss is not cosmetic. A clip at the pane edge takes the **summary tail**
with it — the schedule, the action and the countdown — so a long-named automation
would render as a marker and a truncated name with no indication of when it runs.
That is the pane's entire content.

## 5. Rejected: let a seated pane declare its own block

The frame difference could be closed by giving `PaneDecl` border and title options.
Refused: `App::paint_plugin_pane` is deliberately the single painter for both
placements so that "a seat decides *where*, never *how*" holds
(`plugin-host/panes`). A manifest border field would make the frame the plugin's, and
then a pane could draw itself as focused when it is not — the exact confusion ADR-51
closed by resolving the level from the focus the kernel owns.

The converse — teaching the *native* pane to keep its own block after a handover — is
not available at all: after the handover there is no native pane.

## 6. Rejected: converge the frame inside the handover

Folding the block change into the deletion is tempting (one commit, one snapshot
churn). Refused for the reason ADR-52 gave when `ui::tasks_panel` stopped fitting in
the change that added `ellipsize` rather than in the handover: a handover's claim is
that **which code draws the pane** changed and nothing else did. If the same commit
also restyles the border, that claim becomes unverifiable — a reviewer cannot tell an
intended restyle from a regression, and the acceptance snapshot moves for two reasons
at once.

So the cost is accepted: `empty_welcome_screen_renders` moves twice, once for the
corners here and once for the band's absence at the handover, and each move has one
reason.

## 7. The `Active` level, which was being thrown away

`App::pane_focus_level(KeyContext::Automations)` already returns three levels:
`Focused` while the pane holds the keyboard, `Active` while the central-pane editor
or the run history does, `Inactive` otherwise. The native pane collapses `Active`
into `Inactive` — its `match` is `Focused => border_focused, _ => border_unfocused` —
and the module comment says so, keeping the distinction because "it is real state the
pane could use".

`focus_block` maps `Active` to the plain accent, which is what the ` Sessions ` block
above it has always done. Adopting it therefore *starts drawing* a level the kernel
was already computing: while you edit an automation in the central pane, the band it
came from is accent-bordered rather than grey. That is the correct reading of
`Active`, it is the reading every other pane gives it, and it is stated in the
proposal because it is a visible change that no test would have caught as
interesting.

It does **not** change the rows. `resolve_rows` marks the cursor's row only on
`Focused` or a search preview, and that stays; so does the published
`cursor_visible`, which deliberately excludes the editor focuses so the two panes
cannot disagree about a drawn cursor.

## 8. Consecutive yielding runs, for real this time

ADR-52 specified that consecutive `ellipsize` runs are cut as **one** string, and
justified it with "a searched title is three runs and one string to a reader". The
tasks pane exercised it, but its title is one run except while a search is running.

This pane's name is *always* several runs when a global search matches it, and its
recordings include a case with three matched offsets in a multi-byte name. So this
change is where the rule earns its specification: the ellipsis must fall where
`truncate_ellipsis(name, budget)` would have cut the concatenation, not once per run.
The oracle covers it because the plugin segments the name at the host's byte offsets
and the native builder does the same — if the budget were per-run the two would still
agree, so the *narrow-width frame equality* is the assertion that pins it, compared
against `truncate_ellipsis` through the native tree.

## 9. Regenerating the recordings, and proving nothing else moved

ADR-42 makes the recording the pane's durable expectation and permits regeneration
only from the native builder while it exists. The check that the regeneration did not
smuggle in a defect is a **multiset** comparison of the diff: every removed line must
reappear with `ellipsize` appended to its facts, and no line may appear or disappear.
A per-file eyeball would not catch a row that lost a run.

The alternative — leaving the recordings and accepting that they no longer describe
the pane — is the failure mode ADR-48 exists to prevent, in its subtlest form: the
recording would still pass against the *plugin* (which also gains the flag) and
diverge from the pane.
