# Design

Four decisions, each with the alternative that was available and the reason it
lost.

## 1. The plugin owns its syntax highlighter; the kernel publishes no token stream

A diff line's body is drawn one run per syntax token, coloured by
`ui::syntax::highlight` — comments muted, string literals in the branch colour,
numbers in the working colour, keywords in the accent, capitalised identifiers in
the bright accent, everything else in the primary foreground. Reproducing the row
means reproducing that split.

**Chosen:** the section publishes the line's **text** and its file's **path**, and
the plugin lexes. The bundled plugin carries the lexer in Luau — the keyword
union, the per-extension comment marker, and the four scanners — and the port is
only a port because it does.

**Rejected: publish the body already tokenised**, as an array of
`{ text, token }` runs. It is a smaller change, a shorter plugin and a faster
render. It was rejected for the rule ADR-29 fixed and ADR-27 argued first:
*publish a rendering only when two panes must agree about it.* Nothing else in
thurbox reads `ui::syntax` — `src/ui/code_review.rs` is its only caller — so a
published token stream would be one pane's presentation crossing the boundary, and
the plugin reproducing it would be arranging runs the kernel had already decided.
That is the same objection that stopped ADR-27 from publishing `"8.0/16.0 GB"`,
and it bites harder here: highlighting is the most obviously *presentational*
thing in the whole pane, and a third party writing a diff pane would write their
own highlighter rather than ask for ours.

The cost is real and worth stating: the two lexers must agree token for token, and
nothing but the equality test makes them. A drift in `KEYWORDS` breaks the test
rather than the pane, which is the right way round.

**Also rejected: publish only the comment marker** (`//`, `#`, `--`) so the plugin
need not carry `lang_for`'s extension table. The plugin gets the path anyway — a
diff line without a path is not addressable — and mapping an extension to a
comment style is exactly the kind of decision a pane should own.

## 2. The native renderer is not refactored to draw the tree

The three earlier ports each made the native pane paint the view tree it was
compared against (`info_tree`, `tasks_tree`, `file_tree`), so tree equality was
also frame equality by construction. This port does not, and the departure needs
its reason recorded.

`unified_diff_line` cannot be expressed as a geometry-free tree, and not for one
reason but three: it windows the body to `[h_scroll, h_scroll + avail)`, it slices
that window by **character count** against a width, and the pane's wrap mode
reflows one logical row onto several by the same arithmetic. A tree carries no
width, so a tree built for the plugin can hold only the *whole* body and rely on
the renderer's clip. Forcing the native path through such a tree would change what
the pane paints for double-width text (character count and display width differ),
and would leave the wrap and side-by-side paths as a second implementation of the
same row — the "must stay in lockstep" comment `visual_line_count` already carries,
duplicated.

**Chosen:** add `ui::code_review::diff_row_tree`, a geometry-free builder, and pin
it to the untouched renderer with a **frame** comparison in `ui::code_review`'s own
tests: paint the tree into one buffer, paint `unified_diff_line` into another at
the same width, and require the two buffers to be identical. The plugin is then
compared to the tree builder, and the chain closes on what the pane paints today.

**Rejected: compare the plugin to a new tree builder only.** That is circular — two
functions written in the same change agreeing about a format neither is obliged to
match. It is precisely the shortcut Phase 4 asks a port not to take.

**Rejected: refactor the native path anyway and accept the wide-character change.**
The pane's own tests and any pinned frame would move, and "the port changed what
the pane draws" is a worse outcome than "the port did not touch it".

The consequence to state plainly: this reproduction is validated at the level of
**one row**, painted, plus tree equality for the stream around it. It is not the
claim that the whole pane's rendering is now the tree's, and §11 of the readiness
document says so.

## 3. A tint is a role on the run; the fill is a node

The row background is the *only* thing that distinguishes an insertion from a
deletion in the body — the gutter's sign is one character, and the body's colours
are the syntax highlighter's. So a port without the background is not a port of
"add/remove colouring".

**Chosen:** `TextStyle::tint: Option<DiffTint>`, two members, resolved by the host
to `diff_added_bg` / `diff_removed_bg`, with selection winning. Plus
`ViewNode::Fill { glyph, style }` so the tint reaches the pane's edge.

Why the tint is on the *run* and not on the line: `ViewNode::Line` is a tuple
variant with several construction sites and, more importantly, a line is also the
inline container used *inside* other nodes — a background on the container would
have to compose with a nested container's own, which is a rule nothing needs.
Every run of a tinted row carries the same tint, which is exactly what
`row_bg_fn` does today.

**Rejected: a `StyleToken` for each diff background.** Tokens are foregrounds
everywhere else; a token that means "use me as a background" would make
`token_color` a liar and let any run paint any palette entry behind itself.

**Rejected: infer the tint from the list's selected row** (the way a list already
knows its cursor). The kernel does not know which rows are insertions, and it
should not: that is the pane's reading of its own content.

**Rejected: let the renderer extend the last run's background to the line's end.**
It would remove the need for `Fill`, and it would silently change every existing
pane's last run. A fill is explicit, is reusable for a flush-right run (§8), and
costs one node.

## 4. The stream is published as a bounded window, and the bound is the finding

`MAX_NODES` is 4096 for a whole converted tree. A diff line costs one node for its
gutter, one for its fill, and one **per syntax token** — five to thirty for real
code. So a plugin cannot return a thousand-row diff at all: conversion refuses the
tree and the pane shows an error rather than a shorter diff.

**Chosen:** `MAX_REVIEW_ROWS` caps the published rows, and the plugin returns a
list carrying the cursor's row so the kernel windows what it drew. The cap is
recorded as a *limitation of the model*, not as a design: it is the first pane
whose content is unbounded per row.

**Rejected: raise `MAX_NODES`.** The budget exists to bound the work a plugin can
make the UI thread's renderer do, and a diff is exactly the case where "just make
it bigger" has no defensible number.

**Rejected: have the plugin merge adjacent same-coloured runs.** `ui::syntax`
deliberately does not merge them, so merging would break equality — and it would
only postpone the bound.

**Named, not designed:** the honest closure is that the kernel windows *before*
conversion, which means the plugin would return rows lazily or be told a row
budget. Both are shapes the model has refused for width and height, and one
consumer is too few to design a third. §11 records it as open.

## What was not touched

- `src/app/view.rs`, so the native pane is still what the interface draws and the
  teardown gate's row for this pane stays blocked.
- The native paint path in `ui::code_review`, so no pinned frame can move.
- `tests/architecture_rules.rs`: no new module edge. The one place that sees both
  `ui::code_review` and `plugin::PluginHost` is an integration test.
