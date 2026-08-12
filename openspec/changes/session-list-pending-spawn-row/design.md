# Design

## 1. One field, and its presence is the flag

`SessionRowSnapshot.pending_phase: Option<String>` — the compact phase label
(`fetching…`, `setting up…`, `creating…`, `spawning…`), `None` on every session's row.

A separate `pending: bool` beside a `phase: Option<String>` would admit two states that
cannot happen — a pending row with no phase, a phase on a real session — and a pane would
have to decide which of the two it trusts. `SpawnPhase::short_label` is total, so the
option carries the flag exactly.

**Whether it is running** rides on the status the row already carries: the kernel publishes
`Working` while a background job is churning and `Idle` while the wizard waits at a modal
(`SpawnPhase::is_working`). That is not a pun on the agent's state — there is no agent
yet — it is the kernel's own answer to "is something happening", which is the only thing
the glyph distinguishes. The alternative, a second boolean, would let a row say
`pending_phase = Some(_)` with a status that contradicts it.

**Rejected: reusing `activity`.** It is documented as what the session's *agent* last
emitted, and a spawn has no agent. A pane could not then tell a phase label from a real
activity title, which is what decides both the glyph and the row's shape.

**Rejected: a section of its own** (`PaneContext.pending_spawn`). The placement is the
whole difficulty — the row goes *inside* a repo group, at the end, possibly bringing a
header — so a pane given the row separately would have to re-derive the slot, and it is
not told which repos the spawn will span. Publishing it in place is what makes the
placement the kernel's.

## 2. The slot moves to the model, because it is the ordering rule

`pending_spawn_slot` and `PendingSpawnSlot` go to `src/session/session_list.rs`, beside
`compute_session_order`. They are a pure function of the same data — the rendered rows,
their headers, and the repo names the spawn will carry — and they exist to answer "where
will the real row appear", which `compute_session_order` is the definition of. It is the
same rule ADR-60 moved the rest of the model by, and that change explicitly excluded this
one: *"What deliberately did **not** move is `pending_spawn_slot`, which is downstream of
the very window seam the row below is about."* That seam closed with ADR-63, so the
exclusion is spent, and `migration/handover` now says an excluded part is relocated in the
change that settles the row it waited on rather than in the handover.

`PendingRow` joins `SessionRow` in the same module for the same reason. It carries the
label, the phase and whether a job is running — no width, no glyph, no colour.

## 3. The placeholder becomes an item, so the two trees are comparable

Today the pane builds `Vec<SessionListItem>` and then, separately, `render_session_section`
inserts a placeholder node into the children at the resolved index. The consequence is that
`session_list_tree` — the function the oracle records and the plugin is compared against —
has never seen a pending row, so the recording that must outlive the deletion could not
contain one.

`SessionListItem::Pending(PendingRow)` fixes that at the source: `resolve_items` inserts
the item (and its header, when the group is new) and `session_list_tree` builds the whole
pane. `render_session_section` loses its insertion and finds the placeholder by scanning
the items for the variant, which is also what its hitbox filter needs.

## 4. The spinner is declared, not resolved

`pending_spawn_node` resolved a frame from the caller's `spinner_frame`, with the reason
recorded in its doc: the row "has no identity for a motion lease to key on and the pane
rebuilds it each paint anyway". Both halves are answerable now.

Motion identity is `(pane, node key, signature)` and the key only has to be unique **inside
one pane**; there is at most one spawn in flight, so `pending` is sufficient. And an
identical re-push keeps its epoch, which is the rule that makes a rebuilt-every-paint node
animate at all — it is what a working session's glyph already relies on.

The decisive half is that a plugin cannot be handed a frame. There is no call by which a
pane asks for one, deliberately (ADR-P2a), so a placeholder drawn by a plugin either
declares motion or does not move. A frozen spinner on the interface's only progress surface
is the defect ADR-P12 exists to prevent.

The node is only the glyph, so its signature does not change when the phase label beside it
does: a spawn walking `Branches → Worktree → Spawning` keeps one lease and one phase. It
*does* restart when the flow pauses at a modal, because a non-working phase is a static
`◌` and takes no lease at all — which is correct, since a spinner turning while the app
waits on the user would be a lie.

## 5. Rejected: publishing the placeholder pre-placed as a separate list

A `pending: Option<PendingRowSnapshot>` alongside `rows`, with an index. It is the shape
the native pane has internally (`PendingSpawnSlot { index, header }`), and it would avoid
the "a row that is not a session" awkwardness in the row type.

Rejected because it moves the awkwardness rather than removing it. Every consumer — the
pane, the plugin, the click mapping — would have to merge two lists in the same order to
draw one, and a plugin merging them in a subtly different way would put the placeholder in
the wrong group with nothing to catch it. One list in render order is the shape the section
is defined as ("every row the session list renders, in the order it renders them"), and the
placeholder is a row the session list renders.

## 6. What is deliberately left for the handover

The **click arithmetic**. A handed-over pane's row hitboxes are indices into the published
rows, and `ClickAction::SelectSession` takes an index into the render order — which the
placeholder shifts. The native pane does that subtraction in its own hitbox filter today,
and it stays there: nothing clicks the reproduction, whose pane declares no keyboard, so
moving the arithmetic now would be a change with no consumer and no test.

The **phase fit**. `pending_spawn_node` drops the phase label rather than overflowing a
narrow column, which needs a width. It stays the pane's, like `fit_status_text` beside it,
and dies with the module.
