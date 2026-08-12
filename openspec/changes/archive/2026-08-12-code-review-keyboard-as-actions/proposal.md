# The code review's keyboard becomes rebindable kernel actions

## Why

`tests/code_review_pane_handover_gap.rs` refuses this pane's handover on ten rows. One of
them is named, by the gate's own module doc, as the reason ADR-51's route "does not reach
this pane":

> This pane's keys are **not actions at all**: `KeyContext` has six members and none is a
> review, and `handle_code_review_key` / `handle_review_files_key` are captures keyed on
> `self.focus`, run ahead of the keybinding lookup. So the keys are not rebindable, the F1
> editor has never listed them, and no `keybindings.json` could restore them after a
> handover.

Every other pane thurbox has handed over took the same route: the pane declares that it
**is** the interface's pane for a key context, and the kernel resolves that context's
actions and performs them against its own state. That route is why the file viewer's
handover needed no filesystem capability and no process reach (ADR-58), and why the
automations pane's handover *reduced* a shipped manifest's grants (ADR-56). The route is
unavailable here for one reason only: there is nothing for a declaration to name.

This change writes the names. It is the prerequisite ADR-45 ordered and the gate restates:
**the keys become scoped actions first**, in their own change, and only then is the pane
in the position the other four were already in.

Doing it here rather than inside a handover is the same rule that made the frame converge
first (`migration/handover`, "A pane's frame is converged before its handover"). A commit
that both rewrites a keyboard and moves who draws a pane cannot be reviewed: a lost key
reads equally as a keyboard mistake and a handover regression, and the F1 editor's row
order — which the help modal indexes into — moves for two reasons at once.

It is also a v1 fix that stands on its own. thurbox's largest pane has ~39 keys that the
F1 panel has never listed and no `keybindings.json` can move, in an application whose help
overlay is a live keybinding **editor**. A user who rebinds `j`/`k` everywhere else finds
the review still on `j`/`k`.

## What Changes

- **Two key contexts**: `KeyContext::CodeReview` (the diff) and `KeyContext::ReviewFiles`
  (the changed-files list). Two, not one, because the two panes disagree about `j`, `k`,
  `g`, `G` and `Enter` — the diff walks rows, the list walks files — and one context could
  not hold both meanings.
- **39 scoped actions** replacing the two focus-keyed captures: 28 in `CodeReview`
  (navigation, file/hunk jumps, horizontal scroll, the layout toggles, the comment keys,
  the marks, the target picker, find, export, activate, close) and 11 in `ReviewFiles`
  (navigation, first/last file, open, the marks, find, paging, close). Every one is
  rebindable in the F1 editor and writable to `keybindings.json`.
- **Both contexts join the closed set a pane may declare** (`KeyContext::pane_keyboards`),
  and `App::focus_for_keyboard` maps them to `InputFocus::CodeReview` /
  `InputFocus::ReviewFiles`. This is the whole of what a later handover needs from the
  keyboard; nothing in this change hands anything over.
- **The captures shrink to the sub-modes that own every key** — the target picker, the
  compose box, and the find query while it is being typed. Those stay captures for the
  same reason the file viewer's search field does: while a text field owns the keyboard,
  a letter is text, not a command.
- **The panes stop swallowing unlisted global chords.** A decided behavioural change, in
  both directions, recorded below.
- **`Ctrl+D`/`Ctrl+U` half-paging becomes `d`/`u`.** A decided behavioural change,
  recorded below.
- **The refusal is re-verdicted.** `keys-are-a-capture-not-actions` closes, and with it
  the four rows that named a *power* no capability performs — `no-review-write`,
  `no-retarget-operation`, `no-export-operation`, `no-cursor-write` — because the kernel
  performs each of them on its own rebindable key, exactly as it performs the file
  viewer's directory read. Each of those rows keeps asserting that **no capability was
  granted**, which `migration/handover` requires: "the record of 'the grant was
  unnecessary' is indistinguishable from the grant having quietly happened".
- **The handover stays refused**, on five rows this change does not touch and does not
  claim to: the second seat, the resolved width three layouts divide against, the click's
  column, the anchored compose overlay, and the in-pane multi-line field.

### Decided behavioural changes

Both are visible, so `migration/handover`'s rule applies — a difference is decided, not
discovered.

1. **A global chord fires from the review's panes.** Today `review_escape_chord` lists
   eleven global actions that may pass and the captures swallow every other `Ctrl`/`Alt`
   chord, so `Ctrl+F` (fork) and `Ctrl+R` (restart) do nothing while a review is open.
   After this change the review's panes resolve keys the way every other pane does: a
   scoped action wins, otherwise the global one fires. `review_escape_chord` is deleted
   rather than reproduced — a per-pane allowlist of which global keys work is exactly the
   inconsistency the context lookup exists to remove.
2. **Half-page paging moves from `Ctrl+D`/`Ctrl+U` to `d`/`u`.** The capture *shadowed*
   two global chords, which a declared binding may not do: `macos_default_set_has_no_conflicts`
   asserts the default set holds no chord bound to two actions whose contexts overlap, and
   a scoped chord shadowing a global one overlaps by definition. `d`/`u` are `less`'s own
   half-window keys, are free in both contexts, and are rebindable — a user who wants
   `Ctrl+D` back can capture it in F1, and the editor will say what it took it from.
   Restoring `Ctrl+D` by default would mean a review pane silently eating the delete-session
   chord with the conflict machinery reporting it as a defect.

## Non-goals

- **Handing the pane over.** `src/ui/code_review.rs` is untouched by this change except
  where the two key captures lived. It is still what the interface draws, and
  `the_native_pane_is_still_what_thurbox_draws` still asserts it.
- **Granting a capability.** The bundled `code-review` plugin's manifest is unchanged:
  `capabilities = ["render", "review"]`, `default_visible = false`, no `key_context`. A
  reproduction that took a keyboard for a view it does not draw would be a declaration
  with no consumer — the defect ADR-38 names.
- **Closing the five structural and vocabulary rows.** In particular this change does not
  publish a resolved width, does not add a floating node, and does not widen the click
  event. Those are refused on their own merits and stay refused.
- **The compose box's own keys.** `Ctrl+S` to save, `Tab`/`BackTab` to cycle the
  classification, `Esc` to cancel stay literal, listed under the F1 panel's *Fixed (not
  rebindable)* heading with the other text sub-modes. A modal text field's keys are not a
  pane's keyboard.
- **The mouse.** `ReviewButton` and the footer's click targets are unchanged; a footer
  button still dispatches through `App::cr_button`, not through an action.

## Gate

No new Cargo feature. The work is kernel keybinding vocabulary, present in both build
configurations; `tests/code_review_pane_handover_gap.rs` reads the source rather than
compiling against a feature, so its re-verdict means the same thing with and without
`plugins`.
