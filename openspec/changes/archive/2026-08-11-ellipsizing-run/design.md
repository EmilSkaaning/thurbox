# Design

## What was actually missing

The gates that asked for this named the closure precisely and it has not changed
since ADR-29: *an ellipsizing clip plus a flush-right run*. `ViewNode::Fill` landed
the second half. What remained is a way for a run to say "I am the part that gives
way", because the *kernel* is the only thing that knows how many columns there are —
a fact refused to plugins five times (ADR-26, ADR-29, ADR-30, ADR-31, ADR-39) and not
reopened here.

So the shape is the one the whole catalogue uses: **the plugin declares the intent,
the kernel resolves the geometry.** A gauge declares a percentage and the kernel
sizes the bar; a list declares a cursor and the kernel picks the window; a fill
declares a glyph and the kernel gives it the residue. A run declares that it yields,
and the kernel decides where the `…` falls.

## Decisions

### A style field, not a node kind

`TextStyle::ellipsize`, not `ViewNode::Clip { child }`. Three reasons, in the order
they matter:

1. **It is not a container.** Wrapping a run in a node would let a plugin wrap a
   list, a gauge or a column in it, and none of those has an answer for "ellipsize
   this". The field can only be written where it means something.
2. **A node kind multiplies.** `ViewNode` is walked by `height_of`, `inline_width`,
   `is_inlineable`, `stacked_row_count`, the motion lift, the conversion, and the
   test recorder — all exhaustive on purpose. A field costs one line in the
   conversion and one in the recorder.
3. **Both gates predicted it.** `tests/automations_pane_handover_gap.rs` probes
   `TextStyle` for a clipping flag *and* `ViewNode` for a `Clip`/`Ellipsis` kind,
   because either would have closed its row. Choosing the one those probes already
   read means the gate re-verdicts itself rather than needing a rewritten probe.

The counter-argument is that a `TextStyle` is documented as *how to draw a run* and
truncation is arguably *how much of it to draw*. Accepted, and the doc comment says
so: it is the one field that is neither a colour nor an attribute but a rule for what
happens when the line runs out — which is still a fact about drawing this run and
nothing else.

### Consecutive yielding runs share one budget

The tasks pane's title is not one run. `ui::highlight::highlight_runs` splits it at
the global-search match offsets, so `write it` with a match on `i` is three runs. If
each ellipsized independently, a narrow column would show `w…` `i` `t…` — three
ellipses in one title.

So the resolution walks the line once: fixed runs keep their width, the yielding runs
take what is left in order, and the run that crosses the boundary is truncated with
`…` while later yielding runs draw nothing. That reproduces
`truncate_ellipsis(concatenation, budget)` exactly, which is what makes the native
pane's tree and the plugin's *equal* rather than merely similar.

### The kernel fits with the function the native panes fit with

`ui::truncate_ellipsis`, called from the tree renderer. Not a second
implementation "correct in display width", even though `truncate_ellipsis` counts
characters and is therefore wrong for double-width glyphs.

The reason is what this change is measured by: a plugin's copy of a pane must be
**byte-identical** to the pane. The native panes fit by characters today. A renderer
fitting by cells would cut a CJK title one column differently, and the oracle would
fail on the multi-byte case — reporting the *plugin* wrong for being right. Fixing
the rule is a separate change that moves both panes at once; conflating it with this
one would make a divergence appear in the change that retires one.

### The native tasks pane stops fitting, and consults no dimension at all

`task_rows` loses its `width` argument. This is the load-bearing half: if the native
pane kept fitting in its tree while the plugin declared the flag, the two trees would
differ *by construction* (one carries `write it…`, the other carries `write it all of
it` plus a flag) and no width could make them equal. Moving the fit out of the tree
is what makes the equality assertable at a narrow width — which is the whole
divergence.

The consequence is that `ui::tasks_panel` now reads neither a width nor a height: the
window is the renderer's (ADR-30), and now so is the fit. It is the first pane of
which that is completely true, and it is the shape a handed-over pane wants — a tree
builder with no geometry in it.

### The recordings are regenerated, and that is the rule rather than an exception

ADR-42 requires a pane's oracle to be a **recording** taken from the native builder,
so it survives that builder's deletion, and warns that a `cargo insta accept` can
convert statements about a pane into statements about a plugin. That warning is about
re-recording from the *plugin*. Here the **native** tree changed on purpose, the
native builder is still present to record from, and the diff is one word (`ellipsize`)
on the title runs of twelve files — small enough to read, which is the property the
recorder was made legible for. Asserted rather than claimed: the diff is inspected and
the change reports what it contains.

## Rejected alternatives

- **Report the resolved width into the plugin.** The sixth request, and the sixth
  refusal. It makes rendering width-dependent, so a resize must re-enter a VM before
  the frame that needs it, and a plugin that mis-measured produces a broken pane
  rather than a refused node.
- **Publish the *fitted* title in the snapshot** (the host truncates before the
  plugin sees it). It bakes one pane's geometry into shared state: the tasks section
  is read by any pane, in any column, and the fitting reserves the tasks pane's own
  marker width. It would also mean the published text differs from the record's, so a
  plugin acting on a row would match on a string the kernel invented.
- **A `maxWidth` on the run**, in cells. It is geometry with extra steps: the plugin
  has to know the column width to compute it, which is the thing it is not told.
- **Infer it** — ellipsize the *last* run, or the longest one. Three native panes
  fit a *different* run of their row (the title, the name, the session name), each
  with a different set of siblings to preserve; a rule guessing which would be wrong
  in at least one and unpredictable in all.
- **One ellipsis per yielding run.** Simpler to implement and visibly wrong on a
  searched title, as above.
- **Adopt it in all three panes now.** Each adoption is one line plus that pane's
  re-recording, and each pane's recordings are its handover's evidence. Landing three
  panes' recordings in the change that introduces the mechanism would mean a failure
  in any of them reports "the ellipsis change broke a pane" rather than which pane.
- **Ellipsize inside `ViewNode::Paragraph` too.** A paragraph wraps rather than
  clips, so nothing overflows and there is nothing to cut. Left refused rather than
  silently ignored: the field on a paragraph's run does nothing, and the spec says a
  line is where it means something.
