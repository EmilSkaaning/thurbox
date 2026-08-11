# Design

## The problem, restated precisely

Three bundled pane oracles now exist in three shapes:

| Pane | Oracle | Survives its native builder's deletion? |
|---|---|---|
| info panel | recorded + differential (ADR-42) | yes |
| session list | differential only (`session_list_tree`) | no |
| automations pane | differential only (`automations_tree`) | no |

The two "no" rows are the two panes a handover attempt is aimed at. That is the
whole of the argument for doing them now rather than "each at its own handover":
the recording has to be generated from a builder that exists, and an attempt that
*refuses* the handover leaves no change in which the recording would have landed.

## Where the recorder lives

`tests/` files are separate crates and cannot import one another, which is why
`tests/tasks_pane_input_gap.rs` and `tests/global_search_pane_gap.rs`
deliberately duplicate their source-reading helpers. A **subdirectory** of
`tests/` is not a test target, though, so `tests/view_tree_record/mod.rs`
included with `mod view_tree_record;` is shared code with no new crate, no
`[dev-dependencies]` entry and no build-graph change.

The two cases are not the same and the difference is worth stating rather than
inheriting: a duplicated *reader* of source text is two probes that answer
independently, and drift between them is harmless. A duplicated *recorder* is two
definitions of what an expectation contains, and drift between them is the exact
failure ADR-42 exists to prevent — a fact one pane's oracle stops constraining,
with nothing red.

### Rejected: leave the recorder in `tests/bundled_info_panel.rs` and copy it

Cheapest edit, and it multiplies the one property that has to hold. The
exhaustive destructuring makes a new view-tree field a compile error *in that
file*; two more copies mean the field can be accounted for once and forgotten
twice, leaving two panes recording a tree with a fact missing. The compiler stops
being the thing that keeps the format honest.

### Rejected: a `dev-dependencies` helper crate

A published-shaped crate for one module read by three test files, plus a manifest
entry and a second compilation unit, to avoid a `mod` declaration. It would also
put the recorder outside the tree the architecture rules police.

### Rejected: record the painted frame instead of the tree

Both panes already have a frame-equality test where one is needed (the
automations pane paints at a scrolling height). A recorded *frame* would be a
cell grid whose diffs point at columns rather than at nodes, and it would bake in
a theme and a width — so a palette change would rewrite every expectation and a
reviewer would be diffing wallpaper. The tree is the pane's content; the frame is
one rendering of it.

## What each pane's recording covers

Each pane's existing `cases()` list is the case set, unchanged: the recording is
generated per case with the case's own name, so a failure names the variant.

The session list's cases carry a group header, nested children, remote and
worktree marks, a search-matched run, a selection fill and — the part a weaker
oracle would have exempted — the **spinner as a declared motion node**, whose
key, rate, frames and looping the recorder prints. The automations pane's cases
carry the composed `<schedule> · <action> · <when>` summary, the enabled marker,
the cursor's row and the dimming a search applies.

The **enumerated divergences** get no recording, and each pane's file says so
where the divergence is pinned. They live in their own tests and assert
*inequality* — the session list's empty pane (native tree `ViewNode::list([])`
against the plugin's two centred lines), its non-ASCII whitespace trim, its
windowing rule, the automations pane's fitted name. A recording states what a
pane should be; attaching one to a case that exists in order to differ states
nothing, and recording the *plugin's* side of a divergence is what ADR-42
refuses outright.

## The failure has to be readable, or the compactness bought nothing

ADR-42's argument for a compact recording is that an expectation nobody can read
is one every update rubber-stamps. The same argument applies to the **failure**,
and the first perturbation run showed the oracle failing through
`assert_eq!(plugin, native)` — two thousand-character structural dumps on one
line each, which is precisely the form the recording exists to avoid, arriving at
the moment a reader needs the opposite.

So the shared recorder also owns `assert_matches`, which compares the two
recordings line by line and reports the first line that moved:

```text
the plugin does not reproduce the recorded pane for `one row per status`
line 3:
  native:     text "── thurbox " muted
  plugin:     text "── thurbox " secondary
```

Each pane's per-case loop now asserts in the order *recording → legible →
exact*: the recorded edge first (the fact about the pane), the readable
comparison second, and the structural equality last. The exact assertion is kept
rather than replaced — the recorder is exhaustive by construction but it is still
a projection, and the port's original claim was about the trees — but it is the
one a reader reaches only when the readable one has already named the line.

Lines are compared **by position**, with no realignment after an insertion. An
inserted node shifts everything below it, and the honest report of that is "they
stopped agreeing at line N" plus both line counts; a cleverer alignment would
name a later, prettier difference and bury the real one.

## What this change is careful not to be

It is not a handover, and it is not a step whose value depends on one happening.
Both native panes stay exactly what `src/app/view.rs` draws, so
`tests/teardown_gate.rs` is untouched and both rows stay blocked. If either
handover is refused again, the recordings still constrain the plugins — that is
the point of a recording that is not a difference.
