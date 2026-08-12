# Design — the code review's keyboard as kernel actions

## The shape of the problem

`src/app/code_review.rs` holds two key handlers, both keyed on `self.focus` and both run
from `App::handle_key` *before* `KeyBindings::lookup_in`:

```rust
if self.handle_code_review_key(code, mods) { return; }   // InputFocus::CodeReview
if self.handle_review_files_key(code, mods) { return; }  // InputFocus::ReviewFiles
```

Each begins with `review_escape_chord`, an allowlist of eleven global actions that may
fall through, and ends by swallowing everything else. So the pane's ~39 keys exist nowhere
in the keybinding vocabulary: `Action` does not name them, `KeyContext` has no review
member, the F1 editor cannot list them, and `keybindings.json` cannot move them.

## Two contexts, not one

`KeyContext::CodeReview` and `KeyContext::ReviewFiles`.

One context was the first thing tried and it does not work: the two panes disagree about
five keys. In the diff, `j`/`k` walk **rows** (a diff line, a comment, a hunk header),
`g`/`G` reach the first and last row including the trailing summary, and `Enter` edits the
comment under the cursor or folds the file. In the changed-files list, `j`/`k` walk
**files** with the diff following, `g`/`G` reach the first and last *file* (deliberately
not the summary), and `Enter` drops focus into the diff. A single context would have to
resolve one chord to one action and then branch on focus inside the dispatcher — which is
the capture this change removes, wearing an action's name.

Two contexts is also what a later handover needs: the review is two panes in two columns,
and `plugin-host/manifest` already allows one manifest's two panes to name two different
keyboards.

**Rejected: one `Review` context with focus-branching dispatchers.** It halves the action
count and reintroduces exactly the defect. A user rebinding "next item" would move it in
both panes at once with no way to say which, and the F1 editor would show one row for two
behaviours.

**Rejected: reusing `KeyContext::FileViewer` for the changed-files list.** The list is the
file-viewer *column*'s occupant and its `j`/`k`/`g`/`G` read alike, so the shortcut is
tempting. It is wrong twice: the actions differ (`FileViewerExpand` opens a file from
disk, `ReviewFilesOpen` moves focus), and a plugin declaring `FileViewer` would then also
be declaring the review's list — two surfaces, one declaration, with the seat preemption
rule (ADR-58) already deciding which is on screen. A pane cannot be told it is sometimes
the other pane.

## 39 actions

28 in `CodeReview`, 11 in `ReviewFiles`, one per behaviour the capture had. The count is
the pane's, not this change's: the review is thurbox's largest surface and every key is
documented in `CLAUDE.md`. Dropping any would be a regression hidden inside a refactor.

Naming follows the existing scoped families (`TasksNext`, `FileViewerDown`), prefixed
`Review*` and `ReviewFiles*` so the two contexts read apart in the keymap file.

**Rejected: only declaring the "important" keys and leaving the rest literal.** A key left
literal cannot be given back to a handed-over pane, because the declaration a pane makes
names a context and not a keystroke — so a partial vocabulary would guarantee a later
partial handover, which is the outcome the gate exists to prevent.

## Where the capture survives

Three sub-modes stay captured ahead of the lookup, in one function
(`handle_code_review_submode_key`):

- the **target picker**, an overlay whose `j`/`k`/`Enter`/`Esc` are the selector's;
- the **compose box**, a multi-line text field where every letter is text;
- the **find query while it is being typed**, likewise.

This is the file viewer's rule verbatim — `focus_key_context` already falls back to
`Global` while `file_viewer.search_active` — and the reason is the same: a letter typed
into a field is not a command. `migration/handover`'s new requirement states it, and the
F1 panel lists these under *Fixed (not rebindable)* with the other text sub-modes.

**Rejected: making the compose box's `Ctrl+S`/`Tab`/`Esc` actions too.** Those are a modal
text field's keys, not a pane's keyboard; they follow the automation and task editors,
which are also literal.

## Two decided behavioural differences

### A global chord now fires from the review

`review_escape_chord` is deleted, not reproduced. Today `Ctrl+F` (fork) and `Ctrl+R`
(restart) do nothing in a review because the capture swallowed them; after this change
they behave as they do in the tasks pane, the automations pane and the session list.

**Rejected: keeping the allowlist as a `dispatch_action` guard.** It preserves today's
behaviour exactly, and it preserves a rule nobody can state — eleven global actions work
in this pane and the rest silently do not, with no user-visible reason for the split. The
context lookup exists so that "which keys work here" is answerable from one table.

### Half-paging moves from `Ctrl+D`/`Ctrl+U` to `d`/`u`

The capture bound `Ctrl+D` and `Ctrl+U`, which are `DeleteSession` and
`OpenRestoreSessions` globally. As a *declared* default that is a conflict:
`contexts_overlap(Global, CodeReview)` is true by construction, and
`macos_default_set_has_no_conflicts` asserts the default set holds no chord bound to two
overlapping actions. The conflict is real rather than bookkeeping — the keymap would report
"one will be ignored" to every user, about a chord thurbox itself shipped.

`d` and `u` are `less`'s half-window keys, free in both contexts, and rebindable: a user
who wants `Ctrl+D` captures it in F1, and the editor reports what it was taken from. That
is the visible, reversible version of what the capture was doing invisibly and
irreversibly.

**Rejected: shipping `Ctrl+D`/`Ctrl+U` and relaxing the conflict test.** The test is the
only thing standing between the default keymap and a shadowing nobody can see; relaxing it
for one pane retires it for all of them.

**Rejected: leaving half-paging out.** `PageUp`/`PageDown` alone loses a pager convention
on the pane most likely to be paged, and on laptops `PageDown` needs `Fn`.

## Resolution order, unchanged

`lookup_in` already prefers a scoped action over a global one, so no ordering work is
needed: `d` resolves to `ReviewPageDown` in the diff and to nothing global anywhere else,
`Esc` resolves to `ReviewClose`, and `Ctrl+L`/`Ctrl+Q`/`F7` keep working because they are
global actions no review action shadows.

`Esc` keeps its two-step behaviour inside one action: `ReviewClose` clears a committed
search when one is active and closes the review otherwise. Two actions would put the
pane's internal state into the user's keymap.

## Module ownership

Nothing new crosses a module boundary, which is why `tests/architecture_rules.rs` needs no
edit:

- `Action`, `KeyContext`, `contexts_overlap`, `help_sections` — `src/session/keybindings.rs`
  (pure data, no crate-internal references).
- `dispatch_code_review_action` / `dispatch_review_files_action` — `src/app/code_review.rs`,
  beside the `cr_*` methods they call. `app` already imports everything.
- `focus_key_context`, `handle_key` — `src/app/key_handlers.rs`.
- `focus_for_keyboard`, `pane_chrome`, `KeyContext::pane_keyboards` — unchanged files, two
  new arms each.

`ui` gains nothing and loses nothing: the review's renderer does not know about actions.

## Why the pane keyboards grow now rather than at the handover

`KeyContext::pane_keyboards()` gains both contexts, and `App::focus_for_keyboard` maps
them. Neither is consumed by a shipped manifest — the bundled `code-review` plugin keeps
`capabilities = ["render", "review"]` and declares no `key_context`.

That is deliberately *not* the "capability with no consumer" defect ADR-38 names. A
capability with no consumer is reach a plugin holds and cannot use; this is the kernel's
own answer to "which of my panes is this", and its consumer is the kernel: the four rows
this change re-verdicts are true only if the route actually terminates somewhere. A
`pane_keyboards()` that stopped short would leave the verdict resting on a route with no
last step.

The risk it opens — a third-party pane declaring `CodeReview` — is closed by construction
rather than by a check: `App::session_ring` pushes `CodeReview` and `ReviewFiles` only
while `self.active_review().is_some()`, so a pane declaring either is a focus stop exactly
while a review is open, and receives nothing otherwise. That is the conditional-surface
rule the manifest delta states.

**Rejected: withholding both from `pane_keyboards()` until the handover.** It is one line
smaller and it makes the four re-verdicts unprovable — the row would have to say "the
kernel would perform this if a pane could declare the context", which is a promise rather
than a fact, and the gate's whole design is that a verdict is re-derived from the source.

## What this does not close, and why the handover stays refused

Five rows, unchanged and re-stated in `tests/code_review_pane_handover_gap.rs`:

| Row | Why the keyboard does not reach it |
|---|---|
| `no-second-seat-for-the-changed-files-list` | The file-viewer seat is *preempted* by this very list (ADR-58). A plugin-drawn review needs the seat its own other half takes away. |
| `no-resolved-width` | `v`, `w` and `←`/`→` each divide or chunk against a width a view tree does not carry. Giving the kernel the key does not give the plugin the width. |
| `mouse-carries-no-column` | `click_side` is a coordinate, not a target kind. The keyboard has no bearing on it. |
| `no-anchored-overlay` | The compose box anchors to the selected **row** of a tree the kernel did not lay out — the kernel knows the screen row of each *plugin* row and not which plugin row is the review's cursor. Giving the kernel the `c` key does not give it the anchor. |
| `no-in-pane-text-field` | Its find half has a route (the file viewer's `PaneChrome::SearchBar`, kernel state on a kernel key). Its compose half is the multi-line body inside the overlay above, so the row narrows rather than closes. |

The first four are why this change is a prerequisite and not a handover. The fifth is why
it is narrowed in the gate rather than deleted from it.
