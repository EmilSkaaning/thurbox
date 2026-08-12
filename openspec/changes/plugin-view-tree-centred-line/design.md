# Design

## 1. A node, and it is not a duplicate of `Line`

`ViewNode::Center(Vec<ViewNode>)`, owned by `src/session/view_tree.rs` beside the nodes it
is a sibling of. `crate::ui::plugin_pane` paints it, `crate::plugin::view` converts it. No
new module, no new architecture edge, `tests/architecture_rules.rs` untouched.

The obvious objection is that it is `Line` plus a placement rule, and that the catalog's
own discipline refuses a second spelling of an existing container. The answer is that the
catalog already has exactly this shape: `Line` and `Paragraph` take the *same* children
and differ only in what happens when they run out of room — one clips, one wraps. A third
node taking the same children and differing in *where the row sits* is the established
grain rather than a novelty, and the alternative (§2) is worse.

Its rules follow `Line`'s wherever there is nothing to decide:

- **Children**: exactly what a line admits, asked of the converted child through
  `first_non_inlineable`, so a `column` smuggled inside a `motion` frame is refused with
  the column named.
- **Height**: one row. It clips rather than wrapping, so its height does not depend on the
  width it is given — which is what keeps a centred row from pushing its siblings down.
- **Inlineable**: no. Its width comes from the area rather than from its content, which is
  the whole point of it, so putting one inside a line is refused for the reason a `gauge`
  is.

## 2. Rejected: an alignment field on `Line`

The model's answer. `ViewNode::Line { runs, align }` says what is true — alignment is a
property of a line — and would give right-alignment a second, clearer spelling for free.

Rejected on cost against content. `Line` is a tuple variant at 48 sites across
`src/session/view_tree.rs`, `src/plugin/view.rs`, `src/ui/plugin_pane.rs`,
`src/ui/project_list.rs`, `src/ui/code_review.rs` and `tests/view_tree_record/`, of which
21 are patterns rather than constructions. A change whose whole content is one placement
rule would arrive as a 48-site mechanical rewrite that touches three native panes and the
golden recorder, and reviewing it means checking that every one of those sites still means
what it meant. The recorder is the sharp end: it prints node shape, so a variant that
renamed its field would move recordings for reasons that have nothing to do with
centring — and those recordings are the only evidence four handed-over panes have.

Nothing here forecloses it. If a second alignment consumer appears, `Center` collapses
into the field in a change whose content is that collapse.

## 3. Rejected: an alignment on `TextStyle`

The gate's probe accepts either a centring node **or** an `align` field on `TextStyle`, so
this was on the table.

Rejected because it is not true of a run. Two runs on one line could declare different
alignments and the host would have to arbitrate, or silently let the first win. Every
other field of `TextStyle` describes how *that run* is drawn; alignment describes where
the whole row sits. `ellipsize` is the near miss worth naming — it is a per-run field that
is really a rule about the line — and it earns that because a line may hold several
yielding runs sharing one budget, which is a genuinely per-run fact. Centring has no such
reading.

## 4. Rejected: `fill, run, fill`

A plugin can already put a `Fill` on either side of a run: `inline_spans` splits the
residue evenly between placeholders. That is centring, to within one column.

Rejected because "to within one column" is the wrong kind of nearly-right. The remainder
goes to the **first** placeholder — `share = residue / n`, `extra` handed out from the
left — where ratatui's `Alignment::Center` computes `(width - line_width) / 2` on the
left and so leaves the odd column on the **right**. A pane built this way is off by one
whenever the residue is odd, which is half of all widths, and it is off by one against the
native pane it is reproducing. Changing `Fill`'s remainder rule to fix it is refused
outright: that rule is load-bearing for the diff row's tint and the group header's
trailing rule, and it would move frames in panes that have nothing to do with this.

So the placement arithmetic is the kernel's, and it is ratatui's own — `Layout` with a
`Length` centre and two `Min(0)` shoulders is *not* used, because that is a second
implementation of a division the widget already does; the node hands its row to a
`Paragraph` with `Alignment::Center`, which is the same call
`ui::project_list::render_empty_sessions` makes today. Two panes centring by one call
cannot disagree.

## 5. The constructor, and what it is not

`ui.center` joins the loop in `crate::plugin::capabilities::build_ui_table` that already
builds `row`, `line`, `paragraph` and `column` — a one-word addition to a list of node
constructors — and `src/plugin/bundled/thurbox.d.luau` declares it, because
`scripts/dev/lint-luau.sh` type-checks the bundled plugins in strict mode and a node with
no declared constructor is a node no bundled pane can use.

Stated explicitly because the file is `capabilities.rs`: this grants **no capability**.
`Capability` is unchanged, `build_module_table`'s bindings are unchanged, and nothing a
plugin may read, write, run or reach moves. The `ui` table is the node vocabulary, and it
is frozen after construction — `the_ui_table_is_frozen_too` — so a plugin cannot add to it
either. The spec requires a constructor for exactly this reason: the alternative is a node
kind reachable only by spelling a raw table, which the view-tree spec has refused for
every node since the line.

## 6. What a reader should not conclude

That the session list's empty state is now reproduced. It is not: the native pane still
returns early and draws a `Paragraph`, and the bundled plugin still emits two left-aligned
rows. What changed is that the words *could* be centred by a pane that is drawn from a
tree, which is what the row asked for.
