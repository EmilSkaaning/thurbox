# Design — the handover precondition, and why the gate had to move

## 1. The blocker, stated so it can be checked rather than believed

The task was to make the bundled `info-panel` plugin *the* info panel: flip
`default_visible = true`, keep `F2` / `Ctrl+B` and `[features] info_panel`, stop
calling the native renderer from `src/app/view.rs`, and delete
`src/ui/info_panel.rs`.

The chain that stops it has one link per fact, and every link is already
load-bearing somewhere else in the repository:

| Fact | Where it is enforced |
|---|---|
| a bundled pane is Luau | `src/plugin/bundled/info-panel/init.luau` |
| running Luau needs `mlua` | `Cargo.toml`, `plugins = ["dep:mlua"]` |
| `mlua` is optional and not default | `Cargo.toml`, `default = []` |
| the default build must not gain it | the `plugins` CI job asserts `cargo tree --edges normal` has no `mlua`, and it is a required check |
| the *release* must not enable it | `release/workflow-invariants` specifies it; `scripts/dev/lint-workflows.sh` invariant 2 enforces it over `cd.yml` |

So the pane a user installs cannot be drawn by a plugin. Deleting
`src/ui/info_panel.rs` makes `F2` open an empty column on every release, while
`cargo nextest run --all --features plugins` stays green — the failure mode is
invisible in exactly the build that is tested hardest and absent from the one that
ships.

This is a property of the *build*, not of the info panel. It blocks all seven
Phase 4 panes identically, which is why the condition is expressed once and
applied to every pane row.

## 2. The gate permitted the mistake, which is why this is a change

`tests/teardown_gate.rs` derives a pane's readiness from a conjunction:

```rust
probe: |root, id| bundled_plugin_exists(root, id) && !view_draws_native_pane(root, id),
```

Run that against the tree the task described — plugin present, `view.rs` no longer
naming `info_panel` — and both conjuncts hold. The row becomes `ready`,
`recorded_verdicts_match_the_tree` *requires* it to be recorded ready, and
`every_listed_path_survives_until_its_unit_is_ready` then stops protecting
`src/ui/info_panel.rs`. The gate would have signed off the deletion of the pane
every user is looking at.

That is the same class of error the gate's own module documentation names as its
reason to exist: "A *cleanly* deleted one is silent… The quiet cases are exactly
the ones a table can catch." The existing probe catches the loud case (a pane
reproduced while still drawn) and misses the quiet one (a pane handed to a runtime
the user does not have).

The fix is one more conjunct, `plugin_host_reaches_the_installed_build`, reading
`Cargo.toml`'s `default = [...]` for `plugins`.

## 3. Why `Cargo.toml` is the right thing to read

Three candidate probes, and the choice matters because a probe that answers the
wrong question is worse than none — §10 and §11 of
`docs/PHASE4-PANE-READINESS.md` each record a probe that had to be tightened after
reporting a row closed on a technicality.

**Chosen: the crate's default feature list.** It is the single declaration that
decides whether a released binary can run a plugin at all, it is the same fact CI
and the release lint assert from the other direction, and it is textual — so the
verdict is identical whether the gate itself was compiled with `--features
plugins` or without. That last property is not incidental: the gate's whole design
premise is that "probes read the source tree the way a human auditor would, which
keeps the gate free of the `plugins` Cargo feature".

**Rejected: `cfg!(feature = "plugins")`.** It answers "was *this test binary*
built with the feature", which under `cargo nextest run --all --features plugins`
is `true` — the answer that permits the deletion. It would make the gate's verdict
depend on how the gate was invoked, which is the one thing the gate must not do.

**Rejected: asking whether `render_plugin_panes`' call site in `src/app/view.rs`
is `#[cfg]`-gated.** More local, and it would keep working if the host ever became
unconditional while `plugins` stayed opt-in. But that state cannot arise — the
gate is the *dependency*, not the call site, and the call site's attribute is a
consequence — so the probe would be matching a proxy, and matching an attribute's
position relative to a function is exactly the brittle string-scanning this file
avoids elsewhere. The reasoning is recorded here so a later reader can move the
probe if the premise changes.

## 4. Rejected ways to do the handover anyway

**Flip `plugins` into the default feature set (Stage B).** This is the honest
resolution, and it is not this change. It raises the effective MSRV from 1.86 to
1.88 (`mlua` declares it, and cargo cannot express a per-feature MSRV — the CI
comment says so), it puts `mlua`'s vendored Luau C sources in the path of four
release targets including a cross-built `x86_64-unknown-linux-musl` and a
cross-compiled `aarch64-apple-darwin`, and it contradicts a *specified* release
invariant plus a required CI assertion. Landing it as a step inside a pane port
would be a release-engineering decision made silently. It also has an exit
criterion of its own that has not been met — Stage B asks for at least one plugin
thurbox did not write.

**Ship the info panel's replacement ungated.** The suggestion is coherent for a
pane whose replacement is Rust; it is empty here. The replacement *is* a Luau
program, and an ungated Luau program needs an ungated VM, which is the previous
alternative. Rewriting the pane's plugin in Rust to dodge the gate would produce
`src/ui/info_panel.rs` under another name, and would destroy the only thing the
port measures: that a third party could have written this pane.

**Draw the plugin's pane under `#[cfg(feature = "plugins")]` and the native one
otherwise.** Nothing is deleted, so nothing is handed over, and the cost is real:
two renderings of one pane that differ by build, with the equality test able to
compare trees but never the two frames a user might see. It also fails the phase's
own rule about shortcuts a third party could not take — no third-party plugin can
arrange to be the fallback in a build without a plugin host.

**Delete the native renderer and accept the gap until Stage B.** Named because it
is what the task literally asked for. It would ship a release where `F2` opens an
empty framed column, the settings flag toggles nothing, and the only build that
can draw the pane is one no installer produces. The instruction that governs this
work is explicit that a pane half-replaced is worse than one not replaced; a pane
replaced only in an unshipped build is the extreme of that.

## 5. What is *not* blocked by the release decision, and is still missing

The release blocker would hide these, so they are recorded now rather than
discovered by whoever attempts the handover after Stage B. None is closed here:
each is only useful once a plugin pane can reach a user, and this phase has twice
refused to design a mechanism from one blocked consumer (`thurbox.format.*`, the
non-pane extension point).

| Handover requirement | Where the host stands |
|---|---|
| **the same seat** | `PaneSlot` is a closed set whose only member is `Right`; the info panel is `RegionId::Info`, a distinct region with its own `Percent(15)` share and its own ≥120-column rule. A plugin pane cannot be seated there, so its frame would be a different rect with a different title |
| **the same toggle and flag** | `Action::ToggleInfoPanel` toggles `App::show_info_panel` and `[features] info_panel` gates it; a plugin pane's visibility is `TogglePluginPane` plus a per-pane stored choice. No manifest field asks a pane to answer a kernel action or ride a kernel feature flag |
| **the same latency** | the render worker polls on a fixed 1 s cycle (`PLUGIN_RENDER_SLICE` × `PLUGIN_RENDER_SLICES`). The info panel is the pane with live CPU and memory gauges and per-automation countdowns, so a handover makes the *user's* pane the stale one — the defect `docs/PHASE4-PANE-READINESS.md` §13 says would make a pane feel broken if the plugin owned what the user watches. Here it would |

## 6. The proposed proof cannot fail, and that is worth recording

The task named the acceptance snapshots as "the strongest possible proof": if the
plugin renders identically, replacing the renderer changes no frame.

For this pane the test is vacuous. There are seven snapshots
(`src/app/snapshots/`), and every one is a welcome screen or a modal captured with
no active session — while `App::render_info_panel` returns early when there is no
active session, and the pane is seated only at ≥120 columns. None of the seven
contains an info-panel row: grepping them for the pane's own labels (`Name:`,
`Branch:`, `Agent:`, `Context:`, `Hooks:`) finds nothing. So the snapshots would
have stayed byte-identical if the pane had been deleted outright and replaced with
nothing at all.

The oracle that *can* fail for this pane already exists and is what the port
relies on: `tests/bundled_info_panel.rs` asserts the plugin's view tree equals
`ui::info_panel::info_tree`'s across content variants, and `ui::info_panel`'s own
tests paint that tree against the retained pre-port renderer. The delta on
`migration/phase-4` writes this down as a requirement on a handover's evidence,
because "the snapshots did not move" is a claim a reader will accept without
checking whether the snapshots could have moved.

## 7. Ownership and the architecture rules

No `src/` change, so nothing to declare: no new module, no new type, no new edge
in `tests/architecture_rules.rs`. The gate is a test that reads files as text and
imports nothing from the crate, which is why it can assert a fact about the
default build while running in either configuration.
