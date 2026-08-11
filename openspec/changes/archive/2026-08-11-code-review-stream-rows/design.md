# Design

## The split this change is built on

The pane's out-of-scope list read as one list of ten missing things. It is two:

| Entry | Kind | Why |
|---|---|---|
| file headers (rule, chevron, status, counts, `✓`) | document | rows the kernel already lists; the rule is a `Fill` |
| hunk headers (`@@` ranges, heading, `✓`) | document | facts on the hunk |
| comments + classification badges | document | rows the kernel already interleaves |
| review summary header + summary comments | document | rows the kernel already appends |
| informational rows | document | text the kernel authors |
| reviewed marks | document to draw, behaviour to **toggle** | the mark is a fact; `r`/`R` is a write |
| find bar | behaviour | a text sub-mode that moves the kernel's cursor |
| target picker | behaviour | selecting an entry runs `git` on a worker |
| side by side | behaviour | pairs two halves against a resolved width |
| wrap / horizontal scroll | behaviour | chunk a body against a resolved width |

Only the document column is closable by publishing facts, and this change closes
all of it. Splitting the list this way is the finding: five of ten entries were
blocked by one line in `App::build_review_snapshot` (`let ReviewRow::Line(..) =
row else { continue }`), not by anything about the plugin surface.

## Decisions

### The section publishes one ordered, tagged row list

**Chosen.** `ReviewSnapshot::rows: Vec<ReviewRowSnapshot>`, an enum over the six
kinds, in `CodeReviewState::rows` order.

*Rejected: parallel sections* — `files`, `hunks`, `comments` as separate arrays with
the pane interleaving them. The order is kernel view state: a reviewed file folds to
its header alone (`is_file_folded` = reviewed XOR override), a comment sits after the
line it anchors to, the summary section follows every file. A pane rebuilding that
would be recomputing a decision from a projection of it, and would get folding wrong
the moment the fold rule changed.

*Rejected: keeping `lines` and adding a second `rows`* — two spellings of the same
stream, one of them a subsequence of the other, with nothing making them agree.

### The row tag is `row`, and the line's `kind` keeps its meaning

**Chosen.** Each Luau row carries `row = "file" | "hunk" | "line" | "comment" |
"summaryHeader" | "info"`; a line row still carries `kind = "add" | "del" |
"context"`.

*Rejected: `kind` as the row tag, renaming the line's to `side`* — `kind` is a
published wire name with a shipped reader (`src/plugin/bundled/code-review`), and the
snapshot's promise is that a pane written against it keeps working. Two tag fields
with distinct names cost one line of documentation; renaming one costs every reader.

### A row's glyphs are the pane's, its facts are the kernel's

**Chosen.** The snapshot carries `status = "modified"`, `folded`, `reviewed`,
`classification = "issue"`; the pane maps those to `M`, `▸`/`▾`, `✓`, `[issue]`.

*Rejected: publishing the composed glyph* — the diff **sign** already sets the
precedent in the other direction: `+`/`-`/` ` is derived in the pane
(`signAndTint`), and no one has argued the two panes might disagree about it. A
per-kind marker is the smallest possible rendering, and the rule the snapshot states
is that a rendering crosses only when two panes must agree about something they
cannot both derive. They can both derive a glyph from a wire name.

### The summary header's *label* does cross

**Chosen.** `row = "summaryHeader"` carries `label`, and the pane draws it verbatim.

The native text is `── Review summary (s to add) ──`. It names a keystroke. A pane
composing it would print a hint for a key it never receives — `s` reaches the native
review's own capture, and a focused plugin pane resolves no review action at all. The
two honest options are to publish the label or to draw a different row; publishing it
keeps the reproduction a reproduction, and the exception is narrow enough to state as
a rule (see the added kernel-state requirement).

*Rejected: dropping the hint in the plugin* — the equality would fail, and a
reproduction that differs from the pane it reproduces is the thing this whole port
exists not to be.

*Rejected: publishing every row's composed text* — that is the general case the
exception is carved out of, and it would turn the review section into a rendering
channel.

### A comment crosses as its first line plus a `more` flag

**Chosen.** `text` is `body.lines().next()`, `more` says whether there are further
lines; the pane appends ` …` when `more`.

*Rejected: publishing the whole body* — two costs. A body is bounded at 64 KiB and a
row draws one line of it, so the wire would carry three orders of magnitude more than
the pane needs, per comment, on every publication. And `str::lines()` strips a
trailing `\r` while a Luau split on `\n` does not, so the two panes would disagree
on a comment written on Windows — a divergence with no upside.

### The builder takes snapshot rows, not native diff types

**Chosen.** `review_stream_tree(&[ReviewRowSnapshot], cursor, num_w)`, fed by
`CodeReviewState::snapshot_rows()`, which is also what `App::build_review_snapshot`
publishes.

*Rejected: keeping `diff_stream_tree(&[(String, DiffLine)], …)`* — it made the test
map the same fixture twice, once into native types and once into the snapshot, so an
equality failure could be a bug in the test's own mapping. With one extraction the
plugin and the kernel builder read the *same* published rows, and the claim that ties
either to the pane is the second link — painting the builder's row against the
untouched native renderer — which this change extends to every row kind rather than
weakening.

### Two tokens are added rather than reusing `added`

**Chosen.** `StyleToken::DiffAdded` / `DiffRemoved`, resolving `palette.diff_added` /
`palette.diff_removed`.

*Rejected: reusing `StyleToken::Added`* — it resolves `palette.tool_allowed`, and its
own doc comment says the two are separate fields a custom theme may set
independently. Reusing it would pass every test on the presets that happen to set
them alike and diverge on a theme that does not, which is the failure mode the token
vocabulary exists to prevent.

*Rejected: publishing a colour* — a plugin naming an RGB value cannot follow a theme
switch, which is why tokens exist at all.

### The truncation divergence is enumerated, not closed

**Chosen.** Record that a hunk header, a comment, an informational row and the
summary header clip at the pane edge where the native row ellipsizes one column
earlier; attribute it to the missing resolved width.

*Rejected: publishing the pane's width* — the pane's width is the fact wrap,
horizontal scroll and side-by-side all need, and handing it over is a decision about
the whole view-tree model (every pane becomes a geometry problem, and a pane that
mis-measures a double-width character paints over its neighbour). Spending that
decision on an ellipsis would be paying the model's largest price for its smallest
symptom.

*Rejected: a `truncate` node kind* — the same thing wearing a smaller hat: the node
would carry no width either, so the kernel would have to ellipsize at paint time,
which is a renderer change dressed as a vocabulary change. Worth revisiting **with**
the width decision, not before it.

*Rejected: not recording it* — the existing diff-line equality already holds only for
bodies that fit (`unified_diff_line` windows the body to the available columns), so
the divergence is already there, undocumented. Naming it is a correction, not a new
cost.

## Consequences

- The section's bound now counts headers and comments too, so a review with many
  small files publishes fewer diff lines than before at the same bound. That is the
  bound doing its job: the node budget is spent on whatever rows are published, and
  a header costs nodes.
- The cursor is now the review's own row rather than the nearest published diff line,
  so the plugin's copy scrolls to the header or comment the user is on. Before this
  change, selecting a file header moved the native cursor and the plugin's did not
  move at all.
- The plugin's `render` grows a dispatch over six row kinds; its capability list is
  unchanged at two, which is the property that keeps it evidence about what a third
  party can build.
