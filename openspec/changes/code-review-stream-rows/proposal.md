# Publish the review's whole row stream, so the plugin draws the document

## Why

The bundled `code-review` plugin reproduces the native view's diff **lines** and
nothing else. Its own test names what is missing and asserts it absent rather than
approximated: hunk headers, file headers (the rule and the fold chevron), comments
and their classification badges, the review summary, reviewed marks, the find bar
and the target picker — plus the three width-dependent layouts.

That list mixes two kinds of thing, and the port could not see the difference
because it published one of them:

- **Rows of the document.** A file header, a hunk header, a comment, the summary
  header and an informational line are rows the native pane *lists*, in an order
  the kernel decides — a reviewed file folds to its header alone, a comment is
  interleaved after the line it anchors to. Nothing about them needs a width, a
  keystroke or a repository. They are absent only because `ReviewSnapshot` carries
  `lines`, and `App::build_review_snapshot` skips every row that is not
  `ReviewRow::Line`.
- **Behaviours.** Marking a file reviewed, composing a comment, retargeting the
  diff, finding in it, wrapping it, pairing it side by side. Each needs a host
  power the plugin surface does not have.

So the pane's out-of-scope list is closable exactly as far as the *document* goes,
and this change closes that far. What is left is behaviour, which the next change
itemises with the reason each entry is blocked.

The gain is not "more rows drawn". A diff stream without its file headers is not a
smaller reproduction of the review, it is a different document: the reader cannot
tell which file a line belongs to, cannot see that a file is folded, and cannot see
that a hunk has been reviewed. The row kinds are what make the stream readable, and
they are the half of the pane a plugin can hold today.

## What Changes

- **The review section publishes rows, not lines.** `ReviewSnapshot::lines`
  becomes `rows`, a tagged list in the order the native pane lists them: `file`,
  `hunk`, `line`, `comment`, `summaryHeader`, `info`. The cursor stays an index
  into that list, so it now names the row the review's cursor is *actually* on
  rather than the nearest published diff line.
- **Each row carries what the pane cannot derive, and no rendering.** A file row
  carries its path, its status as a wire name, its insertion and deletion counts,
  whether it is folded and whether it is reviewed — the chevron, the status glyph
  and the `✓` are the pane's. A hunk row carries the two starts, the two spans
  (computed over the hunk's whole line list, which a bounded window does not
  contain) and its heading. A comment row carries its classification as a wire
  name, its body's first line and whether there are more lines.
- **One row's label crosses as text, and it is the exception that proves the
  rule.** The summary header reads `── Review summary (s to add) ──`: it names a
  **kernel keybinding**. A pane composing that string would advertise a key it
  cannot receive, so the kernel authors it and the pane draws it.
- **Two style tokens are added**, `diff_added` and `diff_removed`, for the file
  header's `+n`/`-n` counts. The existing `added` token resolves the palette's
  `tool_allowed`, which is a different field a custom theme sets independently.
- **The kernel's builder covers every row kind**, and each is pinned to the
  untouched native renderer by painting both — the same two-link chain the diff
  row already uses, extended row kind by row kind.
- **One divergence is enumerated rather than closed.** The native pane truncates a
  hunk header, a comment, an informational row and the summary header to the pane's
  width with a trailing `…`; a geometry-free tree carries the whole text and the
  renderer clips it. The two agree on every row that fits and diverge in the last
  column on one that does not. That is the *same* missing width that blocks wrap,
  horizontal scroll and the side-by-side layout, so it is recorded against that
  gap rather than as a fourth one.

## Impact

- `src/session/pane_context.rs` — the review section's row model and its bound.
- `src/app/code_review.rs` — one pure extraction from `CodeReviewState::rows`.
- `src/app/mod.rs` — `build_review_snapshot` calls it.
- `src/plugin/kernel_state.rs` — the `review` table's rows.
- `src/session/view_tree.rs`, `src/ui/plugin_pane.rs` — the two new tokens.
- `src/ui/code_review.rs` — the row-kind tree builders and their paint equality.
- `src/plugin/bundled/code-review/{init.luau,plugin.toml}`,
  `src/plugin/bundled/thurbox.d.luau` — the pane and its declared types.
- `tests/bundled_code_review.rs` — equality across every row kind; the absent-surface
  test keeps only what is still absent.
- `docs/PHASE4-PANE-READINESS.md`, `docs/ARCHITECTURE.md`.

Not changed: the native paint path, the capability list (`render` + `review`, still
two), and what the pane can *do* — this change draws a document and confers no
action.
