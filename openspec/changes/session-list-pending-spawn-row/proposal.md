# The row for a session that does not exist yet

## Why

`tests/session_list_pane_handover_gap.rs` holds the session list's **last** row:

> `no-pending-spawn-row` — the placeholder row a spawning session renders as, inside the
> repo group it will land in — the whole non-blocking new-session flow's only progress
> surface (ADR-P12). The published session row carries a name, a status, a group, a depth
> and four flags; nothing says a row is a spawn in flight, and the slot it lands in is
> `ui::project_list::pending_spawn_slot` over `App::pending_spawn`.

It matters more than its size. ADR-P12 made the whole new-session flow non-blocking — the
`git fetch`, the `git worktree add`, the backend ready-up and the spawn all run on workers,
for tens of seconds on a large repo — and this row is the *only* thing on screen that says
the session is coming. A `status_message` was tried and rejected: those expire after five
seconds, so a long `worktree add` went silent partway through and the app looked idle.

A handover that lost this row would restore that exact defect, so a plugin has to be able
to draw it, in the right place, before the pane can go.

## What Changes

- **The placement leaves the renderer.** `pending_spawn_slot` and `PendingSpawnSlot` move
  from `src/ui/project_list.rs` to `src/session/session_list.rs`, beside
  `compute_session_order` — whose rule they mirror, since the slot is "where the real row
  will appear once it lands". `PendingRow` joins `SessionRow` there.
- **The row is published**, in place. `SessionRowSnapshot` gains one field —
  `pending_phase: Option<String>`, the compact phase label — and the publication inserts
  the placeholder at the slot the model resolved, bringing its own group header when it
  opens a repo group with no rows yet. Its presence is the flag: a session's row never
  carries one.
- **The pane builds it from an item like any other.** `SessionListItem::Pending` joins
  `Header` and `Session`, `resolve_items` inserts it, and `session_list_tree` builds the
  whole pane including the placeholder — so the native tree and a plugin's are comparable
  for the first time, and the oracle can record one.
- **Its spinner becomes declared motion**, like a working session's, keyed `pending`. The
  native pane resolved a frame by hand because the row "has no identity for a motion lease
  to key on"; identity is per pane and there is at most one spawn in flight, so `pending`
  is a sufficient key — and a plugin cannot be handed a frame, so a declaration is the only
  way the row animates after a handover.
- **The bundled plugin draws it**, and a recorded case pins it: a spawn landing in an
  existing group, and one opening a group of its own.

## Non-goals

- **A capability.** The row goes into the `sessions` section a pane already reads.
  `Capability` gains no variant and no module binding: a section grows, a grant does not.
- **Publishing the elapsed time or the phase enum.** The row shows one short string, so one
  short string crosses. The full phase message stays in the status badge, which is kernel
  chrome outside any pane.
- **Making the placeholder selectable.** It has no `SessionId`. It occupies a row and is
  neither clickable nor reachable by `j`/`k`, exactly as today.
- **Handing over the session list.** `src/ui/project_list.rs` still draws the pane. The
  click-index arithmetic a handed-over pane needs — a published row is not a session index
  once a placeholder sits among them — is that change's business, and the native pane keeps
  doing it in its own hitbox filter until then.
- **Fitting the phase text for the plugin.** Dropping the phase when the column is too
  narrow needs a width; a plugin is never told one. The native pane keeps its own fit until
  it is deleted.

## Gate

No new compile-time gate. The model and the pane are in every build; the published field
and the Luau binding are behind the `plugins` feature with the rest of the host. Both
builds are verified.
