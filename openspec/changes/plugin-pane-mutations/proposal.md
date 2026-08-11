# Let a pane change the records it draws

## Why

Every host power a plugin holds today is a **read**. `plugin-host/kernel-state`
publishes sessions, metrics, automations, tasks, files and the open review, and
`plugin/capabilities.rs` inserts one reader per grant; the only writable thing in
a plugin's environment is its own key/value namespace. So the five reproduced
panes can draw thurbox's panes and none of them can *be* one.

The v1 behaviour with no v2 equivalent is the mutating half of two panes'
keyboards:

- the **tasks pane** (`App::dispatch_tasks_pane_action`) cycles a task's status
  with `Space` and soft-deletes it with `d`;
- the **automations pane** (`App::dispatch_automations_pane_action`) toggles an
  automation's enabled flag with `Space`, marks one due with `r`, and deletes one
  with `d`.

A plugin cannot do any of it — not badly, at all: there is no binding, and
denial is by absence, so there is nothing to refuse. A pane replacement's first
condition is that the plugin "handles every key the native pane handled —
selection, scrolling, activation, and any mutating action", and a read-only
plugin cannot meet it.

This is the widest grant surface the host has added, so it is a security decision
rather than plumbing: what follows is a **closed list of five operations**, chosen
as exactly what a native pane performs on one keystroke, each behind its own
declared capability, each denied by the absence of a binding.

## What Changes

- **Two new capabilities**, `tasks-write` and `automations-write`, named per
  record kind for the reason the readers are: the declared set is what an install
  prompt is written from, and "may change your task list" and "may enable and
  trigger your scheduled automations" are different questions to ask a user.
- **Five bindings, and no sixth.** `tasks-write` inserts `setTaskStatus` and
  `deleteTask`; `automations-write` inserts `setAutomationEnabled`,
  `runAutomation` and `deleteAutomation`. Nothing creates a record, edits its
  text, names a command, or reaches another kind.
- **A plugin asks the kernel to run an automation; it never runs one.**
  `runAutomation` marks it due, exactly as the native pane's `r` does, and the
  kernel's own scheduler fires it on its next tick under the claim-CAS that
  already de-duplicates concurrent firers.
- **A mutating binding reaches the database through a seam in the pure-data
  layer**, like the existing plugin store: a trait `session` declares, `storage`
  implements, and each VM builds its own connection **on its own thread**.
- **A pane VM gets a host power for the first time.** The view half is currently
  handed no store at all, so the writer factory is threaded into `PluginHost`;
  the service half gets it too, since a headless plugin closing a task is the
  same grant asked in the same sentence.

## Capabilities

### New Capabilities

- `plugin-host/mutations`: the closed operation list, what each one does and
  refuses, where it runs, what it does *not* grant, and how a plugin's write
  reaches the interface.

### Modified Capabilities

- `plugin-host/capabilities`: adds the two write capabilities, each gating only
  its own bindings, enforced by absence.

## Non-goals

- **No pane is ported.** No bundled plugin declares either capability; the five
  reproductions stay read-only, the native panes stay on screen, every insta
  snapshot stays byte-identical and `tests/teardown_gate.rs` is untouched.
- **No creating or editing.** The native `n`/`e` open the kernel's editor —
  a full-screen surface with its own focus, fields and save semantics that a pane
  cannot own. Granting "create a task with this title" would be a *different*
  feature from the key it is supposed to reproduce, so it waits for whatever
  ports the editor.
- **No cursor, focus, or panel writes.** The state channel stays read-only in
  that direction: a plugin cannot move the user's selection, take focus, show a
  panel, or switch the active session. `docs/PHASE4-PANE-READINESS.md` §10 names
  that as the wall global search hits, and it is still there.
- **No session, git, filesystem or process power.** No spawn, no delete, no send,
  no worktree, no shell. `Capability::Fs` stays undefined, as the teardown gate
  requires.
- **No generic escape hatch.** No SQL, no "call this kernel function", no
  `thurbox-cli` invocation from a VM.
- **No new refresh plumbing.** A plugin's write reaches the panes through the
  `PRAGMA data_version` poll and the ~1 s cache refresh the kernel already runs;
  nothing about a plugin write marks the UI dirty.

## Impact

Behind the existing `plugins` Cargo feature. Nothing in a default build changes:
the seam's trait is compiled (it is pure data in `session/`), and the only
implementor is constructed by the plugin host.

`src/session/plugin_manifest.rs` (two capabilities),
`src/session/plugin_mutations.rs` (new: the seam),
`src/storage/plugins.rs` (the implementor),
`src/storage/automations.rs` (the enable-and-reschedule rule, shared with the
native pane so it has one home), `src/app/automation.rs` (call it),
`src/plugin/capabilities.rs` (the five bindings),
`src/plugin/runtime.rs` + `src/plugin/lifecycle.rs` + `src/plugin/service.rs`
(thread the factory), `src/main.rs` + `src/cli/*` (build it),
`src/plugin/bundled/thurbox.d.luau`, `docs/ARCHITECTURE.md` (ADR-35),
`docs/CONFIG.md`, `CLAUDE.md`.
