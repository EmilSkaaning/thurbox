# A native pane may only be handed over in the build users install

## Why

Phase 4 has reproduced five of thurbox's seven native panes as bundled plugins.
The next step is the one the whole migration is for: stop drawing the native
pane, delete its renderer, and let the plugin *be* the pane. The info panel was
chosen to go first because it is the cleanest possible test of replacement
itself — it is a pure display surface with no selection, no keys and no mouse, so
nothing about interaction can confound the answer to "does dropping a native
renderer leave every frame identical?".

Attempting it produced a different answer, and it is not about the info panel.
**Nothing that a plugin draws reaches a released binary**, so handing any pane
over today would delete a pane from every install. Three facts, each already
enforced somewhere in this repository:

1. `Cargo.toml` reads `default = []`; the plugin host is `plugins = ["dep:mlua"]`.
2. The `plugins` CI job asserts the default dependency tree contains no `mlua`
   ("Stable builds must not gain the runtime"), and it is a required check.
3. `release/workflow-invariants` **specifies** that the release workflow never
   builds with the plugin feature, and `scripts/dev/lint-workflows.sh` enforces
   it over `cd.yml`.

So a bundled pane is written in Luau, a Luau pane needs the VM, the VM is an
optional dependency, and the release provably does not enable it. Deleting
`src/ui/info_panel.rs` would leave `F2` opening an empty column on every
installed copy of thurbox — while `cargo nextest --all --features plugins` stayed
green, because the only build that can draw the replacement is the one nobody
installs.

The v1 behaviour at stake is the native info panel (`src/ui/info_panel.rs`, drawn
from `src/app/view.rs`, seated at `RegionId::Info`, toggled by `ToggleInfoPanel`
and gated by `[features] info_panel`). This change **does not replace it**. It
records why it cannot be replaced yet and makes the reason a gate, because the
gate that exists to prevent exactly this mistake currently permits it.

That last point is the reason this is a change rather than a paragraph in a
report. `tests/teardown_gate.rs` derives a pane's readiness from two conditions —
the bundled plugin exists, and `src/app/view.rs` no longer names the native
renderer module. Both would have been satisfied by the deletion described above.
The gate would have recorded the info panel as *handed over*, and
`every_listed_path_survives_until_its_unit_is_ready` would then have permitted
`src/ui/info_panel.rs` to be deleted — the precise silent case the gate's own
module documentation says it exists to catch. A gate that green-lights the
mistake it was built for is worse than no gate, because it is trusted.

## What Changes

- **No pane is handed over, and no native renderer is deleted.** The info panel's
  plugin stays `default_visible = false`, the native pane stays what
  `src/app/view.rs` draws, and the `info-panel-plugin` row stays blocked — now
  for a second, independent reason that is checked rather than remembered.
- **The teardown gate's pane probe gains a third condition**: the runtime that
  draws a bundled pane must reach the build a user installs. It is read from
  `Cargo.toml`'s default feature list, which is the single fact that decides it,
  and it is the same fact CI and the release lint already assert from the other
  direction. Because the condition is global rather than per-pane, one release
  decision blocks all seven pane rows — which is the real dependency structure,
  and was previously invisible.
- **A test pins the reason**, in the shape `a_reproduced_pane_is_not_a_replaced_one`
  already established: a pane drawn only by a feature-gated build is not handed
  over, so the third condition cannot be "simplified" away without the argument
  surfacing.
- **The worklist's ordering is corrected.** `docs/PHASE6-TEARDOWN-READINESS.md`
  §4 lists Phase 4's seven handovers as step 6 and Stage B — the Cargo default
  flip — as step 7. That order is unbuildable: step 6 cannot complete before step
  7 starts.
- **The audit records the port that did not happen** (`docs/PHASE4-PANE-READINESS.md`
  §14), including three pane-level blockers the release blocker hides, and one
  finding about the proposed proof: the acceptance snapshots cannot witness this
  pane's replacement, because none of the seven contains it.
- **ADR-37** records the precondition as a decision with its rejected
  alternatives, so the next person to attempt a handover meets it in
  `docs/ARCHITECTURE.md` rather than rediscovering it.

## Capabilities

- `migration/teardown` — the pane-handover verdict gains its third condition.
- `migration/phase-4` — "the native pane survives the port" gains the statement
  of what would end that survival, so the phase says when a port becomes a
  handover instead of leaving it implied.

## Non-goals

- **Flipping `plugins` into the default feature set.** That is Stage B: it raises
  the crate's effective MSRV from 1.86 to 1.88, puts a vendored C toolchain in
  the path of four release targets (one cross-built `musl`, one cross-compiled
  `aarch64-apple-darwin`), and contradicts a specified release invariant plus a
  required CI assertion. It is a release-engineering change with its own
  measurement, not a side effect of porting a pane.
- **Closing the three pane-level handover blockers** (a pane slot for the info
  region, the `ToggleInfoPanel` / `[features] info_panel` bindings, and
  event-driven render). Each is only useful once a plugin pane can reach a user,
  and building a mechanism whose only consumer is blocked elsewhere is how this
  phase has repeatedly said not to design.
- **Widening any host surface.** No capability, node, style token, pane slot or
  binding. The info panel's plugin already produces the native pane's view tree;
  nothing about its *rendering* is missing.
- **Retiring or weakening any existing test.** The tightening only adds a
  conjunct; every recorded verdict is unchanged.

## Impact

- Gate: `tests/teardown_gate.rs` (probe + one new test + module docs).
- Docs: `docs/PHASE4-PANE-READINESS.md` §14, `docs/PHASE6-TEARDOWN-READINESS.md`
  §3–§4, `docs/ARCHITECTURE.md` (ADR-37).
- No `src/` change, so no build, no feature gate, and no architecture edge. The
  work lands unconditionally rather than behind `--features plugins`, because a
  gate that only runs in the gated build could not assert a fact about the
  ungated one.
