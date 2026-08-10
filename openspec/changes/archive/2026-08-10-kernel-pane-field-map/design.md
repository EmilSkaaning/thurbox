# Design — the kernel/pane field map

## 1. Why there are no delta specs

`.openspec.yaml` carries `skip_specs: true`. The deliverable is a classification
of state that already exists, produced without moving any of it, and Phase 0 is
explicit that the output is *a map, not a refactor*. There is no behaviour to
require, so there is no requirement a reviewer could point a test at — and the
project's own spec rule ("requirements must be falsifiable") is better served by
declining to write one than by inventing an unfalsifiable one.

What replaces the test is the citation rule in §4: every classification names the
symbol that justifies it, so the map is checkable against `src/` by reading rather
than by trusting.

## 2. Module ownership

None. No type is introduced, no module is added, and `tests/architecture_rules.rs`
is not touched — which is the correct outcome for this change and worth confirming
rather than assuming, since a field map is exactly the kind of artefact somebody
would be tempted to encode as a Rust table.

That temptation was considered and rejected. See §5.

## 3. The three classes, and why three rather than two

The obvious split is two: what stays and what leaves. It does not survive contact
with the struct, because a third of `App`'s fields are neither.

`branch_list`, `worktree_create`, `session_spawn`, `review_build`,
`repo_dir_listing`, `repo_path_check`, `repo_parent_import` and `automation_exec`
are `BackgroundTask` handles; `pending_worktree_create`, `pending_session_spawn`,
`pending_spawn`, `pending_delete`, `repo_picker_gen`, `sync_state` and
`worktree_sync` are the flow state those handles resume into. A pane *triggers*
this work and *renders* its result, which makes it look like pane state. But the
work is git, tmux and SSH — capabilities a plugin will never be granted — and it
outlives the pane: a spawn must survive the session list being hidden, and the
whole point of ADR-P12's non-blocking flow is that the UI is not parked while it
runs.

Filing these as kernel would be true but useless, because it would say nothing
about the thing Phase 4 needs to know: they do not become plugin fields *and* they
do not stay untouched either — each becomes a host call whose result arrives as an
event. That is a distinct migration shape, so it gets a distinct class. It is also
the concrete reason a non-trivial plugin is largely cache management: it cannot
call and wait, so it holds the last answer it was given.

## 4. Every row cites code, and the count is enumerated

Two rules make the difference between a map and an opinion:

- **Each field names the symbol that decides it.** "Kernel because the frame loop
  reads it" is an assertion; "kernel: read by `App::needs_redraw` in the
  demand-driven paint gate" is checkable. Where a classification is contestable
  the citation is what a future reader argues with.
- **The three classes are disjoint and sum to the field count.** A map whose
  columns do not add up is worse than no map, because it invites the reader to
  assume the missing field was considered. The count is taken from the struct, and
  the gated fields are counted separately so the total stays correct in both build
  configurations.

## 5. Rejected alternatives

**Encode the map as a Rust test over `App`'s fields**, in the style of
`tests/teardown_gate.rs` — a table of `(field, class)` rows, asserted complete
against the struct so adding a field fails until it is classified. Genuinely
attractive: it is the one form that cannot go stale.

Rejected for this change, and the reason is not effort. Reading `App`'s field list
from a test means either parsing `src/app/mod.rs` as text — a fragile grep that
would break on the next `#[cfg]` or multi-line generic — or introducing a macro or
derive over `App` to enumerate its fields, which is *changing the thing being
mapped*, in a change whose entire premise is that nothing moves. A staleness
guard bought by editing `App` is not worth it when the classification's purpose is
to be read once per pane port by a human deciding a design.

The honest cost is stated in the doc: the map is a snapshot, tied to a field count
that will drift. It is left as a snapshot deliberately, and the drift is bounded —
Phase 4 is the consumer, and a new field arriving before then is a new field whose
class its author knows.

**Classify only the fields Phase 4 touches.** Rejected: the fields whose class is
obvious are the cheap part, and the value of an enumerated map is precisely that a
reader can tell "not mentioned" from "considered and kernel". A partial map has no
such property.

**Split the doc per pane.** Rejected: the interesting cases are exactly the ones
where two panes want the same field, or where a field looks like a pane's and is
not (`pending_spawn`, `active_index`). A per-pane document hides collisions by
construction.

**Put it in `docs/ARCHITECTURE.md` as an ADR.** Rejected: an ADR records a
decision and its rationale in prose. This is an 85-row table with a tally, and the
repo already puts v2 phase artefacts in their own top-level docs
(`docs/SPIKE-SESSION-LIST.md`, `docs/PHASE4-PANE-READINESS.md`,
`docs/PHASE6-TEARDOWN-READINESS.md`). It follows those.

## 6. Where the map deliberately disagrees with prior prose

An earlier prose design set on the `thurbox-v2` branch contains a document of the
same name, counting 80 fields as 53 kernel / 11 pane / 16 service. This branch's
`App` has 85 fields, and the map re-derives the classification against the code
here rather than porting that table — `openspec/config.yaml` says that design set
is loose reference and not to be ported wholesale. The additions are
`motion_settings` (kernel: the render path reads it every frame, ADR-V18) and the
four `plugins`-gated host fields, which that count predates.

The map also records two refinements the earlier prose does not, both of which are
findings rather than restatements:

- `cached_session_order` is pane state, but the *ordering rule* it caches is also
  consumed by the kernel's own `Ctrl+J`/`Ctrl+K` navigation through a separate,
  uncached call. So a session-list plugin that owns ordering leaves the kernel's
  navigation without a source, which is a Phase 4 obligation rather than a cache
  detail.
- `session_list_state` (pane) sits directly beside `active_index` (kernel), and
  the session-list spike on this branch already concluded the cursor must stay
  kernel-owned. The map states which half of "selection" each field is, because
  the two names read as synonyms and are not.
