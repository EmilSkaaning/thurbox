# Proposal: Event-driven automation triggers

**Status:** Draft / RFC — not yet implemented
**Author:** (proposal)
**Scope:** `session/automation`, `storage/automations`, `app` (status pump), `cli/automations`, `ui/automation_*`

## Problem

Automations are underused. The only trigger today is **time**: an
`AutomationSchedule` is either `Once { at }` or `Cron { expr }`
(`src/session/automation.rs:62-125`). Cron is the wrong primitive for an
*interactive* tool. A manual thurbox user rarely has a recurring prompt they
want fired at 02:00 — they have **moments** they want reacted to: "this agent
just got blocked", "that turn finished, run the tests", "the worktree is dirty,
commit a WIP snapshot". Time scheduling can't express any of those, so the
feature reads as "cron for agents" and most users never create one.

The substrate to react to those moments **already exists** and is already
computed once per tick:

- Session **status transitions** are derived every tick in
  `App::refresh_session_statuses` (`src/app/mod.rs:3531`) →
  `apply_session_status_fields` (`:3625`), which compares the previous
  `session.info.status` against a freshly derived value
  (`derive_session_status`, `:681`). The OS-notification path already consumes
  exactly these edges (`dispatch_status_notifications`, `:3686`).
- The **inter-session message queue** (`src/session/message.rs`,
  `src/storage/messages.rs`, schema v32) gives a durable, exactly-once channel
  for cross-session reactions, with wake nudges and a headless heartbeat drain.

This proposal adds **event-driven triggers** alongside cron, ships a small
**recipe library** so the feature has value the moment it's discovered, and
makes **Exec run history actionable** by linking run rows to the session/diff
they touched.

## Goals

1. Declare automations that fire on a **session status edge** — e.g. "when any
   session in repo X transitions to `Blocked`, send me a Slack-ish nudge", "on
   `Working → Done`, run tests / auto-commit a WIP snapshot".
2. Ship **one-click starter recipes** as data (nightly sync all worktrees, run
   tests on done, auto-commit WIP) so a fresh install has working examples.
3. Make Exec run-history rows **actionable**: link each row to the session it
   reacted to and let the user jump to that session's diff.

## Non-goals

- Arbitrary boolean event algebra (AND/OR across multiple sessions, debounced
  windows, rate windows beyond a simple per-trigger floor). Start with a single
  edge match; compose later.
- Triggers on non-status events (file-watch, git push, CI webhook). The trigger
  enum is designed to extend to these, but they are out of scope here.
- Reliable **headless** edge firing. Event triggers fire from a live TUI
  observer in v1; the headless heartbeat gap is documented under Risks with a
  concrete follow-up.

---

## Design

### 1. The trigger model

Today the `schedule` field is the trigger. Generalize it: an automation's
trigger is one of *time* (existing) or *event* (new). The cleanest fit is to
add a third variant to `AutomationSchedule` (`src/session/automation.rs:62`)
rather than introduce a parallel field, because the persistence, claim, and
`next_after` plumbing all already key off this one enum:

```rust
pub enum AutomationSchedule {
    Once { at: u64 },
    Cron { expr: String },
    /// Fire when a matching session-status edge is observed. Never time-due.
    Event { condition: EventCondition },
}
```

`EventCondition` is a small, serializable struct:

```rust
pub struct EventCondition {
    /// Which session(s) the edge must come from.
    pub source: EventSource,
    /// Which transition(s) match. Empty `from` = "any prior state".
    pub edges: Vec<StatusEdge>,
}

pub enum EventSource {
    Any,                       // every session
    Session(SessionId),        // one pinned session
    Repo(PathBuf),             // any session whose primary cwd is under this repo
}

pub struct StatusEdge {
    pub from: Option<SessionStatus>, // None = any
    pub to: SessionStatus,           // required target state
}
```

`SessionStatus` is the existing five-state enum (`src/session/mod.rs:80`:
`Working`/`Blocked`/`Done`/`Idle`/`Error`). Example conditions:

| Recipe | `source` | `edges` |
|--------|----------|---------|
| Notify on any block | `Any` | `[{from: None, to: Blocked}]` |
| Run tests when this session finishes | `Session(id)` | `[{from: Working, to: Done}]` |
| WIP-commit on finish/block in a repo | `Repo(path)` | `[{to: Done}, {to: Blocked}]` |

The **action** is unchanged — it reuses the existing `AutomationAction`
(`src/session/automation.rs:127`): `Send` (paste a prompt into a session),
`Spawn` (start a session), or `Exec` (run a shell command). An event trigger
just decides *when*; the action decides *what*. This means event automations
inherit the full action surface for free, including the v36 `Exec` path
(`run_exec_command`, `src/session_ops/mod.rs:34`).

### 2. Storage — zero migration

The automations table stores the schedule decomposed into two free-TEXT
columns, `schedule_kind` and `schedule_spec` (`src/storage/schema.rs:104-124`),
written via `AutomationSchedule::kind()`/`spec()`
(`src/session/automation.rs:73-94`). The `Event` variant slots in with **no
schema change**:

- `schedule_kind = "event"`
- `schedule_spec = <JSON of EventCondition>`

`SCHEMA_VERSION` can stay at **36** (`src/storage/schema.rs:15`) — the new kind
rides in the existing free-TEXT columns, written/read via
`AutomationSchedule::kind()`/`spec()`/`from_parts`
(`src/session/automation.rs:75,83,91`).

**Forward-compat caveat (must handle in Phase 1).** Reconstruction goes through
`AutomationSchedule::from_parts` (`:91`), and `map_automation`
(`src/storage/automations.rs:363`) turns a `None` from it into a hard
`FromSqlConversionFailure`. Because `list_automations`/`due_automations`
`.collect()` the mapped rows into a single `Result`, **one unrecognized
`schedule_kind` fails the entire query**, not just that row. So an *older*
binary opening a DB that a newer binary populated with event automations would
break its whole automations list — and even the new binary breaks if
`from_parts` isn't taught the `"event"` kind. Phase 1 therefore must (a) teach
`from_parts` the `"event"` kind, and ideally (b) make `map_automation` tolerant
of unknown kinds (skip the row instead of failing the collect) so future
trigger kinds don't have this footgun. A no-op `SCHEMA_VERSION` bump to v37 does
*not* fix this on its own (it gates migrations, not row parsing); the
row-tolerance change is the real fix.

**Crucially, `next_run_at` is `NULL` for event automations.** The time-based
scanner only selects rows with `next_run_at IS NOT NULL`
(`due_automations`, `src/storage/automations.rs:86`; the partial index
`idx_automations_due` has the same predicate). So event automations are
**invisible to the cron path** — they never appear in `due_automations`, never
get claimed by `claim_due_automation`, and `next_after` returns `None` for them
(consistent with the `Once`-in-the-past case). The two trigger families share
storage but not the firing path.

### 3. Where events are observed

`refresh_session_statuses` runs every tick and is already the single place a
status edge is computed (`src/app/mod.rs:3531`). It calls
`apply_session_status_fields` (`:3625`), where the comparison
`session.info.status != new_status` *is* the edge. Immediately after, it calls
`dispatch_status_notifications` (`:3593`), which walks every session and feeds
`(id, status, is_active)` into `NotificationState::observe`
(`src/app/notify_state.rs:66`) — a pure struct that remembers each session's
prior status and returns `Fire` only on a real, deduped transition.

Event automations plug in **right next to** the notification pump, reusing the
exact same edge source. Add an `AutomationTriggerState` mirroring
`notify_state.rs`:

```rust
// src/app/automation_trigger_state.rs (new, pure + unit-testable)
pub struct AutomationTriggerState {
    prev_status: HashMap<SessionId, SessionStatus>,
    last_fired: HashMap<(i64 /*automation*/, SessionId), Instant>,
}
impl AutomationTriggerState {
    /// Given an observed edge, return the automations that should fire.
    pub fn observe(&mut self, id, prev, new, ...) -> Vec<TriggerHit> { ... }
}
```

and a dispatcher beside `dispatch_status_notifications`:

```rust
fn dispatch_automation_triggers(&mut self) {
    // event automations are cached like cron ones (refresh_automations)
    for session in &self.sessions {
        let (prev, new) = /* same edge already computed this tick */;
        for auto in self.event_automations_matching(session, prev, new) {
            if self.claim_event_fire(auto.id, session.info.id, now) {
                let (status, detail, related) =
                    self.fire_automation_for_event(&auto, session);
                self.db.record_automation_run(auto.id, status, &detail, related);
            }
        }
    }
}
```

This keeps event observation in the same per-tick window as the rest of the
status machinery, so the trigger rule can never drift from the glyph in the
list (the same invariant the notification path already relies on, per the
"observed once per tick … so the rule never drifts from the icon" note in
CLAUDE.md).

Matching is cheap: the cached automation list (`refresh_automations`,
`src/app/mod.rs` ~`:3350`) is partitioned once into event vs. time triggers;
only event ones are consulted here, and only when a session actually changed
status (which is already gated by `changed` in `apply_session_status_fields`).

### 4. Firing + dedup (the claim)

Time automations dedup concurrent firers with an atomic CAS on `next_run_at`
(`claim_due_automation`, `src/storage/automations.rs:185`). Event automations
have no `next_run_at` to swap, and two TUI instances watching the same DB would
both observe the same edge. Two mitigations, layered:

1. **In-process debounce** — `AutomationTriggerState.last_fired` enforces a
   per-`(automation, session)` minimum interval (mirrors
   `min_interval_secs` in `notify_state.rs`), absorbing repeated re-derivations
   of the same edge within one process.
2. **Cross-process claim** — a new `claim_event_fire(automation_id,
   session_id, edge, now, min_interval)` that does a single atomic
   `INSERT … WHERE NOT EXISTS (recent identical fire)` against a small
   `automation_event_fires(automation_id, session_id, fired_at)` table (or,
   cheaper, an `UPDATE automations SET last_run_at=? WHERE id=? AND
   (last_run_at IS NULL OR last_run_at < ?-interval)` CAS, reusing the existing
   column). Only the winner records the run and performs the side effect. This
   is the event analogue of `claim_due_automation` and is the same "SQLite
   serializes writers" guarantee the message queue leans on
   (`claim_messages`, `src/storage/messages.rs:172`).

The action then runs through the **existing** `fire_automation`
(`src/app/mod.rs:4784`) for Send/Spawn and `run_exec_command`
(`src/session_ops/mod.rs:34`) for Exec — with one addition for Exec, below.

### 5. Actionable Exec run history

The run-history table already has a `related_session_id` column
(`src/storage/schema.rs:129`, v28), surfaced in the TUI via
`open_run_related_session` (`src/app/mod.rs:5093`, with a pre-v28 fallback that
scrapes a session id out of `detail`) and `o` on a run row. **Today Exec runs
always record `related_session_id = None`** — `fire_automation`'s `Exec` arm
returns `(status, detail, None)` (`:4830`) because a headless `sh -c` has no
session association.

Event-triggered Exec changes that: the run *was caused by* a specific session's
edge, so we have a session to link. Two concrete improvements:

1. **Link the run to its triggering session.** `fire_automation_for_event`
   passes the triggering `SessionId` as `related_session`, so the Exec run row
   gets a real `related_session_id`. `o` on that row now jumps to the session —
   no behavior change needed in `open_run_related_session`, it just stops
   hitting the `None` branch.

2. **Run Exec in the session's context + expose its diff.** When an Exec is
   event-triggered, run it with `cwd` = the triggering session's primary repo
   and inject `THURBOX_SESSION` (= the triggering session id, mirroring the env
   thurbox already injects at spawn). This makes recipes like "run tests" /
   "auto-commit WIP" operate on the right worktree. The run row already shows a
   tail of stdout/stderr (`render_run_history`,
   `src/ui/automation_detail.rs`); add a small affordance — pressing `d` (diff)
   on a run row whose `related_session_id` is set opens that session's file
   viewer / runs `git diff` in its worktree — so a WIP-commit or test run is one
   keystroke from "show me what changed". `run_exec_command` is extended to take
   an optional `cwd`/env (currently it always inherits the TUI's), keeping the
   no-context call site identical.

This is additive: cron Execs (no triggering session) keep recording `None` and
behave exactly as today.

### 6. Recipe library

Ship a handful of **built-in recipe templates** as Rust data (no schema, no
files) so the Automations pane and CLI can offer "create from recipe" with one
keystroke. A recipe is a named constructor that produces an `Automation`
(trigger + action) pre-filled, which the user can then tweak before saving:

```rust
// src/session/automation_recipes.rs (pure data)
pub struct Recipe { pub slug, pub title, pub description, pub build: fn(...) -> Automation }
pub const RECIPES: &[Recipe] = &[ ... ];
```

Starter set:

| Slug | Trigger | Action |
|------|---------|--------|
| `nightly-sync-worktrees` | `Cron "0 2 * * *"` | `Exec` a worktree-sync command across the watch set |
| `tests-on-done` | `Event {Any, [Working→Done]}` | `Exec` the repo's test command in the triggering session's worktree |
| `wip-commit-on-pause` | `Event {Any, [→Done, →Blocked]}` | `Exec` `git add -A && git commit -m "WIP (thurbox)"` in the session worktree |
| `notify-on-block` | `Event {Any, [→Blocked]}` | `Send` / `Exec` a desktop nudge (complements native OS notifications, but routable to Slack/etc.) |

Surfacing:

- **TUI**: in the Automations pane, `n` (new) opens a small picker — "Blank" or
  one of the recipes — before the editor, so discovery is immediate. The recipe
  pre-fills the editor; nothing is persisted until the user saves (consistent
  with the existing editor flow).
- **CLI**: `thurbox-cli automation recipe list` and `… recipe add <slug>
  [--repo PATH] [--session ID]` instantiate a recipe headlessly. Reuses the
  existing `create` plumbing; `recipe add` is sugar over it.

Recipes are **templates, not magic**: each just produces an ordinary
`Automation` row, fully visible and editable afterward. This keeps ADR-20's
"data, not binary" spirit — the recipe set is a constant list, and a user can
ignore it entirely and hand-author the same row.

---

## Affected modules

| Module | Change |
|--------|--------|
| `src/session/automation.rs` | Add `AutomationSchedule::Event`, `EventCondition`/`EventSource`/`StatusEdge`; extend `kind()`/`spec()`/`from_storage`/`next_after` (returns `None` for events); JSON (de)serialize the condition. |
| `src/session/automation_recipes.rs` *(new)* | `Recipe` struct + `RECIPES` constant list. |
| `src/storage/automations.rs` | No new columns. `due_automations` unchanged (events excluded by `next_run_at IS NULL`). Add `claim_event_fire` (CAS dedup). Optional `automation_event_fires` table if not reusing `last_run_at`. |
| `src/storage/schema.rs` | None required (free-TEXT `schedule_spec` carries the JSON). Optional no-op v37 bump if we want an explicit marker. |
| `src/app/automation_trigger_state.rs` *(new)* | Pure `AutomationTriggerState` mirroring `notify_state.rs` (prior status + debounce). |
| `src/app/mod.rs` | `dispatch_automation_triggers` beside `dispatch_status_notifications` (`:3686`); partition cached automations into event vs. time; `fire_automation_for_event` (links session, sets Exec cwd/env). |
| `src/session_ops/mod.rs` | `run_exec_command` gains optional `cwd`/env; existing callers pass `None`. |
| `src/cli/automations.rs` | `--trigger event:<json>` (or structured flags) on `create`; `recipe list`/`recipe add` subcommands. Headless tick stays time-only (see Risks). |
| `src/ui/automation_editor_modal.rs`, `automation_detail.rs` | Editor: trigger-type selector (Time/Event) + event-condition fields; run history: `d` = open diff for an event run. |
| `src/ui/automation_*` (new-session picker) | Recipe picker on `n`. |
| `docs/FEATURES.md`, `docs/CONFIG.md`, `CLAUDE.md` | Document the event trigger, recipes, and the headless limitation (per the docs rule in CLAUDE.md). |

---

## Phased implementation plan

**Phase 1 — data model + storage (no behavior).**
Add `AutomationSchedule::Event` + `EventCondition` and its JSON round-trip;
extend `kind()`/`spec()`/`from_storage`/`next_after`. Unit tests: an event
automation round-trips through `schedule_kind="event"`/`schedule_spec=JSON`, is
absent from `due_automations`, and `next_after` returns `None`. No firing yet.
*Fully backward compatible; lands independently.*

**Phase 2 — the observer + claim.**
Add `AutomationTriggerState` (pure, unit-tested like `notify_state`) and
`dispatch_automation_triggers` in the tick, plus `claim_event_fire`. Wire
event-Send/Spawn/Exec through the existing `fire_automation`. Acceptance test in
`src/app/acceptance.rs`: drive a session to a `done`/`blocked` hook state via
the harness, assert the matching event automation records exactly one run (and
that a second TUI / repeated tick does not double-fire).

**Phase 3 — actionable run history.**
Populate `related_session_id` for event-triggered Exec; run Exec in the session
cwd with `THURBOX_SESSION`; add the `d` = open-diff affordance in
`render_run_history`. Tests: an event Exec run has a non-`None`
`related_session_id`; `o` jumps to the session; `d` opens its diff.

**Phase 4 — recipe library.**
Add `automation_recipes.rs` + the TUI recipe picker on `n` + CLI `recipe
list/add`. Tests: each recipe builds a valid `Automation`; `recipe add`
persists a row equivalent to the hand-authored `create`.

**Phase 5 — docs + polish.**
Update `CLAUDE.md` (Automations section), `docs/FEATURES.md`, `docs/CONFIG.md`;
record the headless-firing limitation and the follow-up.

Each phase is independently shippable and additive; Phase 1 can merge with zero
user-visible change.

---

## Risks & open questions

- **Headless firing gap (biggest).** Event observation lives in the live TUI's
  tick — `refresh_session_statuses` only runs there. The headless `automation
  tick` (`src/cli/automations.rs:434`, fired by the 60 s tmux heartbeat) has
  **no live status observation**; it only scans time-due rows. So with the TUI
  closed, event triggers don't fire. This is acceptable for v1 (the use cases —
  react to *my* blocked agent, run tests when *I* see a turn finish — are
  inherently interactive), but must be documented. *Follow-up:* the agent hook
  already persists every edge to `sessions.hook_state`/`hook_state_at`
  (`src/storage/sessions.rs:15`), so a headless tick could derive edges by
  diffing the persisted hook state against a per-(automation,session)
  `last_triggered_state` it stores — making event triggers heartbeat-driven and
  TUI-independent. Larger change; deferred.

- **Double-firing across instances.** Two TUIs on one DB both observe the same
  edge. Mitigated by `claim_event_fire` (cross-process CAS) + the in-process
  debounce, but the claim table/CAS design needs care to be genuinely atomic
  (model it on `claim_due_automation` / `claim_messages`, both already proven).

- **Edge storms / flapping.** A session that oscillates `working ↔ done` (the
  normal single-session focus pattern noted in CLAUDE.md) could fire a
  `→Done` automation repeatedly. The per-`(automation, session)` min-interval
  debounce is the guard; its default needs tuning, and possibly an explicit
  `min_interval_secs` field on event automations (reusing the notification
  knob's shape).

- **Semantic stretch of `AutomationSchedule`.** Calling an event condition a
  "schedule" is a naming compromise (chosen to avoid a parallel column + a
  second claim path). Acceptable, but the enum/doc language should say
  "trigger" where user-facing. Alternative considered: a separate `trigger`
  column — rejected as a larger migration for no functional gain, since
  `next_run_at IS NULL` already cleanly separates the two firing paths.

- **Repo-scoped `EventSource::Repo` matching cost.** Matching by repo requires
  resolving each session's primary cwd against the repo path every edge. Cheap
  at thurbox's session counts, but should be a precomputed map if it ever grows.

- **Exec in a worktree is a side effect on the user's tree.** `wip-commit-on-pause`
  literally commits. Recipes that mutate the repo must be clearly labeled and
  default-disabled on creation, and the run-history diff affordance is what
  makes that side effect auditable.

- **Forward-compat of unknown `schedule_kind` (currently a footgun).** As noted
  under Storage, `map_automation` (`src/storage/automations.rs:363`) hard-errors
  on a `schedule_kind` that `from_parts` doesn't recognize, and the `.collect()`
  in `list_automations`/`due_automations` propagates that, so a single event row
  read by an older binary breaks the *whole* automations list. Phase 1 must make
  row parsing tolerant (skip unknown kinds) and lock the contract with a test;
  otherwise downgrading the binary after creating an event automation corrupts
  the user's automations view.
