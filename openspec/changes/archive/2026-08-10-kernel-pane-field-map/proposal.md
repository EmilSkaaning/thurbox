# Map every `App` field to kernel, pane, or service state

## Why

Phase 4 ports thurbox's panes to plugins one at a time. Each port has to answer
"what state does this pane own?", and answering it per pane, at port time, is how
a boundary gets discovered seven inconsistent ways — the first pane sets a
precedent nobody agreed to, and the fifth finds that a field it needs was already
claimed by the second.

`src/app/mod.rs` is 14,958 lines and its `App` struct has **85 fields**, 81 in
every build plus 4 behind the `plugins` feature. Nothing in the tree records
which of them are the kernel's, which leave with a plugin, and which are
kernel-owned work a pane merely *requests*. `docs/PHASE4-PANE-READINESS.md`
already audits what the plugin **API** cannot express; this is the other half of
the same question — what a pane plugin would have to **own**.

The v1 behaviour being extended is nothing in the running program: today every
field is `App`'s, read by whichever method needs it, with no stated ownership at
all. That is precisely why the map has to exist before the panes move, and it is
also why the map is only a map.

## What Changes

- **A new `docs/KERNEL-BOUNDARY.md`** classifying all 85 fields into three
  disjoint classes — kernel, pane, service — enumerated so the columns add up
  rather than being asserted to.
- **The classes are defined by a rule, not by taste.** Kernel: state the kernel
  must own because it outlives, arbitrates between, or routes to panes. Pane:
  state only the owning pane reads and writes, which moves into a plugin's VM.
  Service: kernel-owned work a pane *asks for* — background task handles and
  in-flight flow state — which becomes host calls whose results arrive as events,
  not plugin fields.
- **Every field that does not split cleanly is named as a decision**, with the
  reason, rather than being filed silently on one side. The `modal` enum splits
  by variant; `pending_editor_run` splits into intent and execution;
  `cached_session_order` is pane state whose consumers include the kernel's own
  session navigation; `session_list_state` is pane state sitting next to
  `active_index`, which is not.
- **Every classification cites the code that justifies it** — a file and symbol
  on this branch — so the map can be re-checked against the tree instead of
  believed.
- **The four `plugins`-gated fields are classified too.** They are the host's own
  kernel state, and a map that omitted them would go stale the moment the feature
  is default-on.

## Non-goals

- **No refactor.** No field moves, no type moves, no module changes, no `cfg`
  added or removed. `src/` and `tests/` are untouched, and the map is worthless
  if producing it required changing the thing being mapped.
- **Not a decomposition plan for `App`.** Whether the remaining kernel fields are
  split into sub-structs is a separate question; this says which fields *leave*,
  not how the remainder is arranged.
- **Not the pane migration order.** That is ordered by coupling, and it belongs
  to the phase that does the porting.
- **No feature gate.** A document is not gated; the four plugin-gated fields are
  labelled as such inside it.
- **Not a host-API design.** The map says a service field becomes a host call; it
  does not specify the call.

## Impact

- New `docs/KERNEL-BOUNDARY.md`.
- `CLAUDE.md` and `docs/README.md` — the doc listed where the other design docs
  are.
- No `src/`, no `tests/`, no CI, no behaviour.
