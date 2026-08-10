# Design — the info panel through the view tree

## 1. Where each new type lives, checked against the architecture rules

`tests/architecture_rules.rs` is an allowlist keyed by **top-level module**, so
the question is never "which file" but "which module may see what".

| New item | Module | References | Allowed because |
|---|---|---|---|
| `StyleToken::*` (11 new variants) | `session::view_tree` | `super` only | `session` is pure data; the variants are names, not colours |
| `ViewNode::Gauge`, `ViewNode::Paragraph` | `session::view_tree` | `super` only | same |
| `Percent` | `session::view_tree` | nothing | a newtype over `f32` |
| gauge/paragraph rendering, `token_color` arms | `ui::plugin_pane` | `session`, `ratatui` | `ui → session` is already declared |
| `info_tree` + the section builders | `ui::info_panel` | `session`, `ui` | unchanged from today's file |
| `ui.gauge` / `ui.paragraph` constructors | `plugin::capabilities` | `session`, `mlua` | `plugin → session` is already declared |
| gauge/paragraph conversion | `plugin::view` | `session`, `mlua` | same |

**No new edge**, so no allowlist edit, and no matching change to `CLAUDE.md`'s
dependency block or `docs/CONSTITUTION.md`. In particular `ui` still never
references `crate::plugin`: `ui::info_panel` reaches the *tree*
(`crate::session::view_tree`) and the *renderer* (`crate::ui::plugin_pane`),
neither of which can reach a VM. That is the same property that made
`ui::plugin_pane` legal in the first place, and it is why the info panel can be
ported without the plugin feature being compiled at all.

The two plugin-side files are wholly inside `#[cfg(feature = "plugins")]`
modules already (`src/plugin/` is gated at `src/lib.rs`), so the default build
gains nothing from this change but the kernel-side nodes.

## 1a. The gate: the view tree was plugin-only

Discovered while implementing, not while planning, and it is the change's most
consequential decision. `session::view_tree`, `session::motion` and
`ui::plugin_pane` were all `#[cfg(feature = "plugins")]`. The default build —
the one users install — did not carry them at all.

So the criterion as written was **unsatisfiable**: a pane rendering through the
tree would have needed a second, hand-built renderer for the default build,
selected by a Cargo feature. Two renderers for one pane is precisely the
divergence byte-identity exists to prevent, and it would have diverged silently
in the build nobody tests the feature leg of.

**Decision: ungate all three.** They reference neither `mlua` nor
`crate::plugin`; `cargo tree --edges normal | sort -u` is byte-identical before
and after, so the gate bought a stable build nothing. The view tree is now the
kernel's rendering IR, which is what makes the proposal's claim ("no plugin
runtime is involved") true rather than aspirational.

**Rejected: gate the info panel's view-tree path.** See above — it is the failure
mode, not an option.

**Rejected: rename `ui::plugin_pane`.** The name now misleads, since it paints a
kernel pane. But the rename carries no behaviour and would have buried the
substantive diff (ungating plus two node kinds) under call-site churn. Recorded
as a named follow-up in the module's own doc comment rather than left implicit.

## 2. Why a gauge *node* and not a reported rect

`docs/PHASE4-PANE-READINESS.md` §4 framed the choice and this change takes the
side it named.

**Rejected: report the resolved pane rect back to the plugin.** It is the
general answer and the worse one. Rendering would become width-dependent, so a
resize has to re-enter the VM *before* the frame that needs it — putting plugin
code on the resize path, which is exactly what ADR-V11's "no plugin call on the
render loop" forbids. And a plugin that mis-measures produces a visibly broken
pane rather than a refused node, which moves the failure from a diagnosable
error to a rendering artifact.

**Chosen: a `gauge` node.** Geometry stays in the kernel, the node is trivially
theme-aware, and it bounds what a plugin can ask for. The cost is a less general
catalogue, which is the stated intent — the catalogue is "the set thurbox's own
panes need, not a general drawing API".

**Rejected: build the gauge as a pre-computed `Line` in the info panel.** This
would have worked and needed no catalogue change at all, because
`render_info_panel` is *handed* `area.width` and could pre-resolve the padding
and bar itself. It was rejected because it would have made the exercise
worthless: the tree would carry pre-resolved geometry that a plugin cannot
produce, PHASE4 §4 would have stayed open while looking closed, and the port
would have proved only that a pane with privileged access to its own width can
be expressed.

## 3. Why a `paragraph` node and not wrapping on `line`

**Rejected: a `wrap` flag on `ViewNode::Line`.** It changes the shape of an
existing variant, so every `match` on `Line` in the conversion layer, the
renderer and the tests moves — for a property that is not a variation on a line
but the negation of its defining rule. `Line`'s specification says it "MUST be
clipped at the pane edge rather than wrapped"; a flag that turns that off makes
the requirement meaningless.

**Chosen: a separate `paragraph` node.** Purely additive: no existing variant
changes shape. The two nodes then say two different things, both of which the
pane needs — a `label: value` row must not push its neighbours down when the
value is long *if the value is a fixed-width field*, and must wrap when the value
is agent-supplied prose.

**Rejected: keeping `Line` everywhere and accepting clipping.** This is the one
place the port could have skipped a widening and still produced a working pane,
so it deserves naming: `Activity` and `Signal` carry OSC-derived text of
unbounded length, and a gauge header whose `label + suffix` exceeds the width
overflows. v1 wraps all three today. Clipping them is a user-visible regression
in exactly the rows a user reads when a session needs attention.

**Implementation note.** The paragraph's rows are produced by ratatui's own
`Paragraph` with `Wrap { trim: false }`, and its height by that widget's
`line_count(width)`. This is deliberate: it is the *same* code path v1 used, so
byte-identity across the wrap is a property of construction rather than of a
reimplementation of word-wrapping that would have to be tested into agreement.

`line_count` is gated behind ratatui's `unstable-rendered-line-info` feature,
which this change enables. Weighed against:

- **Reimplementing word-wrap.** Rejected: `WordWrapper` with `trim: false`
  handles whitespace runs, unbreakable words and wide graphemes, and any drift
  becomes a subtle rendering bug in the pane. Measuring with the widget that
  paints cannot drift.
- **Rendering into a scratch buffer and counting the rows written.** Correct and
  stable-API-only, but it doubles the paint work for every paragraph and needs a
  bound on the scratch height that is itself a wrap calculation.
- The risk taken is a semver-exempt API: a future ratatui bump can rename or
  remove it. That surfaces as a **compile error** in CI, not as wrong output, and
  the fallback above stays available. Enabling the feature adds **no crate** to
  the dependency graph (`instability` is already a ratatui-widgets dependency) —
  verified by diffing `cargo tree --edges normal` before and after.

## 3a. What the oracle caught: a gauge is not two rows

The differential test earned its cost on its first run. v1's
`render_gauge_lines` computes `padding = width - label - right` **saturating** to
zero, so when the label and suffix together exceed the width the header line
overflows — and, being inside the pane's wrapping `Paragraph`, it *wrapped*,
pushing the bar onto a third row.

The first implementation of the gauge node had a fixed height of 2 and clipped
that header. The sweep failed at `Context`/`7%` in a 6-column pane, naming the
exact cell. The node's height is therefore `header rows + 1`, and the delta spec
was corrected before the change was archived rather than after.

This is the strongest argument in the change for writing the oracle: the gauge's
own audit entry (PHASE4 §4) asserted "two lines", the plan repeated it, and only
a mechanical comparison against the code being replaced disagreed.

## 4. Why eleven new tokens, and why not a colour

The five existing tokens resolve onto five `ThemePalette` fields. The pane draws
from these additional **distinct** fields, each traced to the row that needs it:

| Token | Palette field | Row that needs it |
|---|---|---|
| `secondary` | `text_secondary` | `Parent`, `Activity`, an automation's label |
| `role` | `role_name` | `Agent` |
| `branch` | `branch_name` | `Repos` and its continuations |
| `added` | `tool_allowed` | `+120` in `Changes` and `Lines` |
| `status_working` | `status_working` | `Status`, when working |
| `status_blocked` | `status_blocked` | `Status` and a highlighted `Signal` |
| `status_done` | `status_done` | `Status`, when done |
| `status_idle` | `status_idle` | `Status`, when idle |
| `status_error` | `status_error` | `Status`; also `-8` in `Changes`/`Lines` |
| `status_unreachable` | `status_unreachable` | `Status`, on a down host |
| `border` | `border_unfocused` | the section separators |

Two notes on the naming. `added` is named for its role rather than for
`tool_allowed`, the palette field v1 happens to draw insertions with — naming the
token `tool_allowed` would export a v1 accident into the plugin API, and naming
it `diff_added` would be a lie, since that is a *different* palette field. And
`status_working`/`status_idle` resolve to the same fields as the existing
`warning`/`success`; the duplication is kept on purpose, because `warning` is a
token a plugin picks for *its own* meaning while `status_working` is the token
for *the kernel's* session status, and collapsing them would leave a plugin
drawing a status indicator picking `warning` for working and `status_blocked` for
blocked. `border` exists because a divider already draws in `border_unfocused`;
naming it lets a separator be a styled text run where a full-width rule is wrong.

**Rejected: letting a node name a colour.** The direction of the constraint is
fixed by the existing specification — tokens exist so that a plugin follows a
theme switch. Eleven more tokens keep that property; one `rgb` field would end
it.

**Rejected: `StyleToken::Status(SessionStatus)`.** Tempting, and legal (both
types are in `session`), but it makes the token set open-ended in a second
dimension: `as_str`/`parse` stop being a flat table, and the Luau-facing wire
name becomes structured (`status:done`) for one token family only. A flat name
per status costs six variants and keeps one shape.

## 5. Enumerated divergences from v1

Byte-identity is asserted cell for cell against the retained v1 line builders
(`legacy_lines`, `#[cfg(test)]`) over a matrix of widths and content variants.
Three divergences survive that comparison. None moves the pinned frame; each has
its own test.

1. **Space cells carry a theme foreground instead of the terminal default.** v1
   used `Span::raw(" ")` between `3 files` and `+120`, and
   `Span::styled("  ", Style::default())` between an automation's countdown and
   its label — spans with *no* foreground set, so those cells recorded
   `Color::Reset`. Rendered through the tree they carry the adjacent run's theme
   colour. A space has no glyph and neither span sets a background, so the two
   are visually identical; the pinned frame cannot see it. Reproducing it would
   have required a token meaning "the terminal's default foreground", which is
   the one thing the token set exists to prevent — on a themed background a
   terminal-default foreground is the colour that can be invisible.
2. **Agent-supplied text is sanitized.** `ViewNode::text` runs
   `sanitize_text`, so a control character in `Activity`, `Signal` or a session
   name is dropped and a tab becomes four spaces. v1 passed the bytes into a
   `Span`, and ratatui writes a cell's symbol to the terminal verbatim — so an
   OSC title containing `\x1b[2J` reached the terminal as a control code. This
   divergence is a fix, not a cost: the pane renders text thurbox does not
   author.
3. **`Status` and `Agent` keep `BOLD`; the section headers keep `BOLD`.** No
   divergence — recorded here only because `TextStyle` carries exactly one
   modifier flag (`bold`) and the pane happens to need no other. A row that
   wanted `ITALIC` would have found a fourth gap.

A fourth candidate was checked and is **not** a divergence: v1's single
`Paragraph` wrapped every line in one widget call, while the port renders one
widget per node. ratatui's word-wrapper processes each input line independently,
so the two produce identical rows — which the differential test over long
content is what actually establishes.

## 6. Why the percentage is a newtype

`ViewNode` derives `PartialEq, Eq, Hash`, and that is load-bearing: an identical
re-push must keep a motion's epoch, or any plugin that re-renders on unrelated
state pins its spinner to frame 0 forever. An `f32` field would remove `Eq` and
`Hash` from the whole enum.

`Percent` therefore wraps the `f32` and compares by `to_bits()`. It deliberately
does **not** normalise at construction: the renderer clamps exactly where v1
clamped, so the port is byte-identical for every input including a non-finite
one, where v1 printed `NaN%` and drew an empty bar. A plugin cannot construct
that case — `plugin::view` refuses a non-finite percentage at conversion — so the
untidy value is reachable only from the kernel's own metrics, which is where it
came from before.

## 7. What this change does *not* settle

- **PHASE4 §2 (no host binding reads kernel state) stays open**, and it is the
  reason the info panel is still not portable to a plugin. The pane receives its
  `SessionInfo` as a function argument; nothing here lets a plugin obtain one.
  So the honest claim is narrow: *the catalogue can express this pane's
  rendering*, not *a plugin could write this pane*.
- **PHASE4 §5 (per-pane keyboard visibility) stays open.**
- The pane still allocates its strings per frame. v1 borrowed `&'a str` from
  `SessionInfo` where it could; `ViewNode::Text` owns its content, so a visible
  info panel now allocates roughly thirty short strings per paint. It is paid
  only when the panel is visible and the frame is dirty, and the pane already
  called `format!` for most of those rows; measuring it was not worth blocking
  the port, but it is a real cost of the tree's owned-content design and the next
  pane's port should watch for it at list scale.
