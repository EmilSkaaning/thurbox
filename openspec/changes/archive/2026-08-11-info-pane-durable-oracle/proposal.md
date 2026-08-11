# The info panel's equality proof outlives the pane it compares against

## Why

The info panel is the next native pane to be handed over: its plugin reproduces
it exactly, it takes no keys, and since ADR-40 the runtime that draws it is in
the build a user installs. The proof that would authorise that handover is
`tests/bundled_info_panel.rs`, which asserts the bundled plugin's view tree
**equals** the one `ui::info_panel::info_tree` builds from the same state.

That proof is **differential**, and the handover deletes the other side of it.
`info_tree` lives in `src/ui/info_panel.rs` — the module the handover removes —
so at the moment the comparison matters most, it evaluates to nothing:

- `cases()` would compare the plugin against a builder that no longer exists, so
  the file would not compile and the natural fix is to delete the assertion;
- what survives is a test that renders the plugin and checks it does not error,
  which is satisfied by a pane drawing one wrong row, or twenty;
- nothing else constrains the pane. The seven acceptance snapshots are all
  captured with no active session, and the info panel needs one, so none of them
  contains a single cell of it. `migration/phase-4` already forbids citing them
  ("The proof a handover offers is checked for being able to fail"), which leaves
  the handover with no oracle at all.

So the pane whose handover is cheapest is also the one whose evidence is about to
evaporate, and the two facts are the same fact: an oracle written as a difference
between two live implementations cannot outlive either of them.

This change makes the info panel's oracle **recorded** rather than differential,
and does it while the native builder is still present — which is the only moment
the recording can be proven to be the *native pane's* appearance rather than the
plugin's. It deletes nothing and hands nothing over: after it, the interface
still draws the native info panel, and the teardown gate still says so.

The same hole exists in the other five bundled pane oracles. This change fixes
the one whose handover is next and states the rule the others will need, rather
than rewriting six oracles for a handover that has not been designed yet.

## What Changes

- **A recorded expectation, captured from the native builder.** One snapshot per
  comparison case, holding a compact line-per-node rendering of the tree.
- **Both proofs run while both sides exist.** The differential assertion
  (plugin == native) is kept *and* joined by a recorded one (native == snapshot).
  The recording is therefore checked to be the native pane's, and the plugin is
  checked against the native pane, in the same run — so when the native builder
  goes, the surviving assertion (plugin == snapshot) inherits a proven baseline
  instead of freezing whatever the plugin happened to do.
- **A formatter that cannot silently narrow the oracle.** The renderer
  destructures every `ViewNode` variant and every `TextStyle` field with no rest
  patterns, so a field added to the view tree fails to compile until the oracle
  accounts for it.
- **Non-vacuity is demonstrated, not asserted.** The change records what
  perturbing the plugin does to the snapshot.

## Impact

- Affected specs: `migration/phase-4` (one MODIFIED requirement, two ADDED).
- Affected code: `tests/bundled_info_panel.rs`, plus a new snapshot directory
  `tests/snapshots/`.
- No `src/` change, no behaviour change, no deletion. The native info panel is
  still what the interface draws, `tests/teardown_gate.rs` is untouched, and the
  info panel's row stays blocked.
- **Test count is unchanged (2707).** The recorded edge is two more assertions
  inside the existing per-case loop, not a new test — which is worth stating
  plainly, since "the suite grew" is the usual evidence that a proof was added
  and here it is absent by construction. What changed is what that one test can
  fail for, demonstrated by perturbing each side in turn rather than by a count.
- The whole change is confined to `tests/`, so the `--no-default-features` build
  is untouched: the file is `#![cfg(feature = "plugins")]` and does not compile
  there, exactly as before.
