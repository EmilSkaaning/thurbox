# Design — the session list as a bundled plugin

## 1. The kernel publishes no rendering of a row, but it does publish the status triple

`StatusSnapshot` already carries a status's glyph and style token, and it does so
for a stated reason: `StyleToken::for_status` exists precisely so two native panes
cannot disagree about which colour a session state gets, and a plugin
re-deriving that mapping would be a second, unchecked copy. This pane reads that
triple and draws it.

Everything else about a row is the plugin's. It picks the single space either side
of the glyph, the `└` / `↳` prefix, `⇅`, `⑂`, the two-space separator, the
accent-bold-underline emphasis on a matched character, and the whole precedence
`selected > dimmed > role`. In particular the kernel publishes `activity` and
`notification` **separately** and does not resolve the blocked-first priority
between them: which of the two a row shows is a presentation decision the pane
makes from a status name it already has.

### Rejected: publish the resolved row text

Publishing one `status_text: Option<String>` per row, already resolved by the
blocked-first rule and already fitted to the column, would make the plugin's row
a re-arrangement of strings the kernel composed. Rejected for ADR-29's rule — the
same reason a task row publishes `status` and not a checkbox glyph. It would also
be unusable: the fit is against the *native* pane's width, and the plugin's pane
is a different rect, so a pre-fitted string is wrong at its own width. This is why
`TaskSnapshot::title` is not fitted either.

### Rejected: publish the spinner frames

`ui::SPINNER_FRAMES` is a rendering. The plugin declares its own ten braille
frames, and the equality test is what proves the two agree — exactly as the tasks
pane declares its own `☐ ◐ ☑` rather than reading them. A published frame list
would make the animation the kernel's and the port's only contribution a `for`
loop.

## 2. Motion: the native pane animates through the same frame table a plugin does

ADR-V18 shipped declared motion with no bundled consumer. The obvious way to use
it here is to give the plugin a `motion` node and leave the native pane on its
existing `spinner: &str` argument — but then the two trees are not equal (one
holds a motion node, the other a text node), and equality is this port's entire
oracle. Weakening the comparison to "equal up to the spinner" would exempt the
one part of the pane that moves.

So the native pane's tree carries the **same** motion node, and its paint
resolves the frame through `FrameTable` — the plain-data channel `ui` already
reads (it is how `ui` can draw an animation without being able to reach a VM,
which `tests/architecture_rules.rs` enforces). The table is filled from
`App::spinner_frame()`, the clock the native spinner already ran on, so no
behaviour changes: same frames, same rate, same reduced-motion answer (frame 0,
`⠋`).

The motion's key is a fixed string (`"spinner"`) in both trees. Identity is
`(pane, key, signature)` and therefore per-pane, so a third-party plugin may use
any key it likes and its pane animates identically; the two agreeing here is an
artifact of comparing trees, and the test says so rather than implying that a
plugin must guess a magic name.

### Rejected: give the native session list a real motion lease

Registering the native pane in `App::motion` alongside the plugin panes would
make one clock drive everything. Rejected as out of scope and as a behaviour
change: `MotionState` allocates a bounded aggregate rate across *leases*, and
adding a native pane to that budget would let an installed plugin's animation
degrade thurbox's own spinner — a regression introduced for tidiness. The frame
table is the seam that gives equality without moving the budget.

## 3. The oracle: the native pane draws the tree, but the list widget stays

The three list ports before this one refactored their native pane to draw its own
tree, and the code-review port did not (its painter is width-dependent in ways no
tree expresses). This pane is in between, and the split is drawn at the row:

- **The rows are trees.** `group_header_line` and `build_session_line` are
  replaced by `session_item_node`, and the native pane paints its nodes through
  the same `inline_spans` walk `ui::plugin_pane` uses — so a `Fill` resolves its
  residue by the same arithmetic in both panes, and the selection bar and the
  group rule reach the same column.
- **The list is not.** Which rows are on screen, where a two-line item starts,
  and which cell a click lands in are resolved by the ratatui `List` widget from
  its own offset. That is left alone. Converting it to `ViewNode::List` would
  change what the pane scrolls like — ratatui's sticky offset and ADR-30's
  `visible_window` keep the cursor visible by different rules — and the hitboxes
  are derived from the offset the widget actually used.

The consequence is one enumerated divergence rather than a weaker claim: the
plugin's pane windows by ADR-30's rule and the native pane by ratatui's, so the
tree comparison runs at a height where neither windows anything, asserted by
`the_comparison_size_adjusts_nothing`.

### Rejected: keep the native paint path untouched and compare frames

The code-review port's shape. Rejected here because it is available *and weaker*:
these rows genuinely are geometry-free once the activity text is fitted, so a
refactor makes the native pane draw the compared tree and turns a two-link chain
into one. Frame equality is still asserted, as a second check on the `Fill`
residue — but it is not the primary claim.

### Rejected: move the fitting of the activity text into the tree

`push_agent_status` measures what the row has used **in characters**, and drops
the text entirely when fewer than four columns remain. Both are geometry. Keeping
them in `resolve_items` follows ADR-29: the kernel owns the pane's size, the tree
owns its appearance.

## 4. Why no new capability

`Capability::Sessions` is documented as "read the sessions thurbox is running —
names, branches, agent metrics, activity text". A reader that returns every row
rather than the active one is the same sentence, and the capability list is what
an install prompt is written from: a user asked "reads your sessions" has already
been told this. Splitting it into `Sessions` and `SessionList` would put two
questions in the prompt for one disclosure and would make a session-list pane
demand two grants.

The disclosure does widen — one session's name and activity becomes every
session's — and that is recorded in the capability's own documentation rather
than hidden behind an unchanged sentence.

### Rejected: a separate `session-list` capability

Rejected as above. Also rejected because it would make `readers_present()`
ambiguous: the publisher builds one snapshot for whichever kinds have readers, and
two capabilities over the same records would have to agree about which of them
fills the section.

## 5. The render trigger, and why it is reported rather than fixed here

The plugin worker renders every pane, then waits out a fixed 1 s interval in ten
100 ms slices, serving key requests. Nothing tells it that kernel state moved. So
when the user presses `Ctrl+J`, the native pane's cursor moves on the next frame
(single-digit milliseconds) and the plugin's *copy* of that cursor moves on the
worker's next cycle — up to 1 s later.

The spike predicted this and named the fix: render on a state change. Two
attempts at it were considered and both are worse than the finding:

- **Nudge the worker whenever the published snapshot changes.** The snapshot
  changes on almost every tick, because it carries host CPU and memory. That
  turns a 1 Hz poll into a ~100 Hz one — a *regression* in idle cost, in exchange
  for latency the user cannot see on a hidden pane.
- **Nudge only when the session section changes.** Better, and probably right
  eventually. Rejected *here* because it is a change to the render loop's
  contract with a rate policy attached (a floor, or bar 1's 10 Hz ceiling is
  breached by an agent that emits activity text quickly), and it belongs to a
  change about the frame budget with its own measurement — not to a pane port,
  which would be the second thing this commit proved.

So the port records it: the measurement, the reason the *user-visible* cursor is
unaffected (selection is kernel state — the spike's second condition), and the
cost of closing it. That is the honest shape of a condition that no longer holds.

## 6. What the port did not need, which is the result

Nothing was added to `session::view_tree`. `Line`, `Fill`, `Motion`, a
`List` carrying its cursor, and the four `TextStyle` fields between them describe
every row this pane draws. Four ports built that surface for panes that were each
easier than this one, and it held for the pane ADR-V1 hinges on without a single
new node, style field, or token.

Two of them earned their keep immediately: `Fill` — added by the code-review port
for a row tint that has to reach the pane's edge — is what the selection bar and
the group header's rule need here, which makes this its second consumer and the
first evidence that it was a general node rather than one pane's escape hatch.
