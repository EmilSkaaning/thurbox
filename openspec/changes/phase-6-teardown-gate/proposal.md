# A machine-checked teardown gate, because Phase 6 cannot start yet

## Why

Phase 6 (`docs/v2/MIGRATION.md` §2) is the destructive phase: delete 5,028 lines
of v1 extension system, delete each native pane behind its plugin replacement,
absorb the built-in hooks behaviour into the kernel, flip the defaults, ship
2.0.0. Its inventory (§4) is prose in a design document, and its precondition —
that every deleted behaviour demonstrably lives somewhere else — is stated
nowhere a build can check.

I audited that precondition against the tree. It does not hold, and it does not
hold *narrowly*: the phase that produces the replacements has not run.

| Phase 6 step | Blocked by |
|---|---|
| Delete each native pane | Phase 4 never ran — `src/plugin/bundled/` contains `hello` and nothing else, so 0 of the 7 panes has a plugin implementation to become the default |
| Delete the v1 extension system | 6 of the 7 "must not be lost" capabilities have no v2 home in the build |
| Absorb hooks into the kernel | `ensure_builtin_hooks_extension` still installs through `install_extension`, so deleting the installer deletes agent status reporting |
| Replace `[features]` flags with `plugin enable\|disable` | The `plugin` CLI has `list`, `status`, `doctor`, `reload` — no `enable`, `disable`, `install`, or `update` |
| Flip the runtime default and ship 2.0.0 | Stage B has not happened: `Cargo.toml` still has `default = []`, so no user has ever run the plugin host |

So the honest outcome of this change is **not a deletion**. It is the inventory,
with the evidence, in a form that a future session cannot skip by reading a phase
label. Deleting working functionality to satisfy that label is the one outcome
worse than doing nothing.

Two things push this past a written note. First, a prose inventory drifts: the
readiness verdicts above are facts about *today's* build and there is no reason
they will still be true when someone reads them. Second, the hooks payloads
already have a real drift hole. `remote_hooks.rs` carries a kernel-side table of
which file each agent's hooks land in, and a test asserts that table matches the
embedded manifest — but only on destination and guard directory, never on the
**payload**. The local install and the remote provisioning can therefore ship
*different hook content* for the same agent, silently, and the thing that breaks
is the status dot: the exact behaviour MIGRATION §4 names as the one that must
not be lost.

## What Changes

- **A teardown gate**, `tests/teardown_gate.rs`, in the allowlist spirit of
  `tests/architecture_rules.rs`: each deletion target is listed with the
  preconditions blocking its removal, each "must not be lost" capability carries
  a recorded verdict, and every verdict is checked against a probe of the current
  source. Deleting a listed target while a precondition is unmet fails the build
  with the unmet list. A verdict that no longer matches its probe fails, naming
  which row to revisit — so the inventory cannot rot into a rubber stamp.
- **The hooks drift hole is closed.** The embedded asset table is hoisted out of
  `materialize_source` into one `EMBEDDED_ASSETS` constant, and the manifest-sync
  test is extended to assert byte-identical payloads: for every agent wired
  through its own config directory, the file the local installer writes and the
  payload the remote provisioner ships are the same bytes.
- **No deletion, and no flag flip.** `default = []` stays. The extension system
  stays. Every native pane stays.

## Impact

- New: `tests/teardown_gate.rs` (the inventory, as a check).
- Changed: `src/session_ops/builtin_hooks.rs` (asset table hoisted, no behaviour
  change), `src/session_ops/remote_hooks.rs` (payload identity asserted).
- Docs: `CLAUDE.md` "Architecture Enforcement" gains the gate;
  `docs/ARCHITECTURE.md` records why the inventory is executable.
- Ungated: the gate is a source-level check with no plugin dependency, so it runs
  in both Cargo configurations.
