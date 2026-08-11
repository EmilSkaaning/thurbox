# Design

See `proposal.md` — Why. This is a grant decision, so it records what each
capability does **not** give as carefully as what it does.

## 1. The five operations, and why exactly these

The rule applied: **one operation per key a native pane performs with a single
keystroke, and nothing else.**

| Native key | Native call | Plugin binding |
|---|---|---|
| tasks pane `Space` | `Database::set_task_status` | `setTaskStatus(id, status)` |
| tasks pane `d` | `Database::soft_delete_task` | `deleteTask(id)` |
| automations pane `Space` | get + recompute + `update_automation` | `setAutomationEnabled(id, on)` |
| automations pane `r` | `Database::trigger_automation_now` | `runAutomation(id)` |
| automations pane `d` | `Database::delete_automation` | `deleteAutomation(id)` |

The three keys deliberately left with **no** binding are the ones whose native
handler opens a *kernel surface* rather than changing a record:

- tasks `n` / `e` and automations `n` / `Enter` open the central-pane editor — a
  full-screen surface with its own focus, fields and save semantics. A
  `createTask(title)` binding would let a plugin do something the key does not do,
  and still not reproduce the key. It waits for whatever ports the editor.
- tasks `r` opens the trigger-time action picker, which spawns or prompts a
  *session*. That is session power, and it is not in this change at any width.
- tasks `o` switches the active session — a focus write, which
  `docs/PHASE4-PANE-READINESS.md` §10 records as structurally blocked.

So a pane replacement built on this change can reproduce `Space` and `d` (and
automations' `r`), and its `n`/`e`/`r`/`o` are still open. That is stated here
rather than discovered at port time.

## 2. What each capability deliberately does not grant

**`tasks-write`** cannot: create a task; change a title, description, source or
external id; restore a deleted task; read the task list (that is `tasks`); touch
an automation, session, file or review; or run anything.

**`automations-write`** cannot: create an automation; change a schedule, action,
command, prompt or target; read the automation list (that is `automations`); spawn
a process or a session from the plugin's thread; or name a program.

**Neither** grants: SQL, a generic "call a kernel function" binding, a
`thurbox-cli` invocation, another plugin's key/value namespace, the filesystem
(`Capability::Fs` stays undefined — `tests/teardown_gate.rs` reserves it), git, or
any write to *view* state (cursor, focus, panel visibility, active session).

## 3. The residual reach, stated plainly

`automations-write` is the widest grant in the host, and it is worth naming the
exact shape rather than the comfortable summary:

- An automation's action may be `Exec`, which runs a shell command **the user
  wrote**. A plugin holding this capability can cause that command to run.
- What bounds it is that the plugin cannot *author* one: with no create and no
  update binding, the set of programs it can trigger is exactly the set the user
  already scheduled. That is the whole mitigation, and it is a real one — but it is
  not "this capability cannot run code".
- Enabling a disabled automation is in the same class: it will then fire on its own
  schedule.
- One asymmetry survives: a user presses `r` once, and a plugin may call
  `runAutomation` on every render cycle. Marking a *pending* run again is
  idempotent (the write is "next_run_at = now", not a queue), so the ceiling is one
  fire per kernel pass rather than one per call — but it is a ceiling, not a
  refusal. A rate limit was considered and rejected for now: it would put a clock
  and per-plugin state into a seam that has neither, for a plugin the user chose to
  install and could equally annoy them with a `while true` render loop. If a bound
  is added later it belongs with the host's other execution bounds, not here.

## 4. The seam: a trait in `session`, a connection per thread

Mirrors `session::plugin_store::PluginStore` exactly, because the same two
constraints apply: `plugin` may not import `storage` (architecture rule), and a
`rusqlite::Connection` cannot cross threads.

```text
session::plugin_mutations::KernelWriter        (trait, pure data layer)
session::plugin_mutations::KernelWriterFactory (Arc<dyn Fn() -> Option<Box<dyn KernelWriter>>>)
storage::plugins::DbKernelWriter               (the only implementor)
```

**One trait for both capabilities**, not two. The trait is host-side plumbing, not
the grant surface: what a plugin holds is decided by which *bindings* are
inserted, and the spec's enforcement rule is about the binding's absence. Two
traits would double the factory threaded through `runtime`, `lifecycle`, `service`,
`main` and three CLI call sites to express a distinction the capability check
already makes.

Rejected: **routing a mutation to the UI thread** as a request/reply, the way a key
is routed. It would make a write depend on the frame loop (and on a frame
happening), and the write has no answer the UI needs. The store precedent already
established that a VM may hold its own connection.

Rejected: **`thurbox-cli` as the mutation surface** (a plugin shells out). It would
grant process execution to get a status change, which is exactly backwards.

## 5. A pane VM gets its first host power

`PluginThread::spawn` passes `None` for the store today, so a *view*-half VM has
no storage binding at all; only `ServiceHost` builds a factory. The writer factory
therefore has to be threaded into `PluginHost` as well — the first time a pane's
VM holds anything but readers and its own view tree.

This is worth noticing rather than doing quietly: it means a pane plugin's reach is
no longer bounded by "it can only produce a tree", and the review question for a
bundled pane changes accordingly. The grant is still per manifest and still
absent-by-default.

`PluginHost` must stay `Send` (startup runs on a worker), which the `Arc<dyn Fn +
Send + Sync>` factory shape already satisfies — the same shape the store uses.

## 6. Enabling shares one implementation with the native pane

`App::toggle_automation_by_id` reads the automation, flips `enabled`, recomputes
`next_run_at` from the schedule, and writes the row. `Database::set_automation_enabled`
does *not* recompute — its doc says the caller sets `next_run_at`.

So a plugin binding calling the storage method directly would leave an enabled
automation with no next occurrence, which is a subtly dead automation. The rule
moves into `Database::set_automation_enabled_rescheduled`, and the native pane
calls it too. One rule, one home — otherwise "a plugin toggling an automation" and
"a user toggling an automation" are two behaviours with one name.

## 7. Module ownership, against the architecture allowlist

| New/changed | Module | Allowed |
|---|---|---|
| `KernelWriter`, `KernelWriterFactory` | `session::plugin_mutations` | `session` references nothing |
| `DbKernelWriter` | `storage::plugins` | `storage` already implements `PluginStore` here |
| `set_automation_enabled_rescheduled` | `storage::automations` | uses `session::automation`'s schedule, which `storage` may |
| the five bindings | `plugin::capabilities` | `plugin` → `session` only; the trait is in `session` |
| the factory's construction | `main`, `cli::*` | already construct the store factory the same way |

No allowlist entry changes.

## 8. What is not in this change

- **No bundled pane declares either capability.** Snapshots do not move and the
  teardown gate is untouched; the port that uses this is a separate change.
- **No `plugin doctor` section.** Unlike a spawn contribution, a mutation has no
  static verdict to re-derive: the manifest's capability list already says who may
  write, and `plugin status` prints granted capabilities today.
- **No confirmation prompt.** A cross-process prompt-and-wait channel does not
  exist (`AgentPolicy`'s missing `confirm` value is the same gap), and inventing
  one for this would be a second, worse version of it.
