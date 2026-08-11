# Design — the plugin host in the default build

## The shape of the change

One line of `Cargo.toml` decides it. Everything else here exists because that
line makes four other statements false, and each of them is currently *enforced*
somewhere — by a manifest field, a required CI step, a specified invariant, or a
serde default. The work is finding all four and handling each deliberately, not
flipping the flag.

```text
default = []            →   default = ["plugins"]
rust-version = "1.86"   →   rust-version = "1.88"      (mlua's floor)
clippy.toml msrv        →   1.88
CI: assert no mlua      →   assert mlua               (direction inverted)
invariant 2: no feature →   invariant 2: no --no-default-features
hello: no seed (=true)  →   default_visible = false
```

## The measurement, since this is the part that can go wrong

`mlua 0.12` → `mlua-sys 0.11` → `luau0-src 0.20.7+luau728`, whose build is
`cc::Build::new().std("c++17").cpp(true)` over Luau's `Ast`, `Compiler`, `VM`,
`Config` and `Common` sources. So the new build requirement is a **C++17 compiler
and, when cross-compiling, a C++ standard library for the target**. It is not
merely "a C toolchain": `rusqlite`'s bundled SQLite already required a C compiler
on all four targets, and that is why none of them needed one added.

Measured on this machine (4 cores, `rustc 1.97.1`, `--release`, `lto = true`,
`codegen-units = 1`):

| | `thurbox` | `thurbox-cli` |
|---|---|---|
| `default = []` | 12,015,688 B | 9,847,304 B |
| `default = ["plugins"]` | 14,457,432 B | 12,244,376 B |
| delta | +2,441,744 B (+20.3%) | +2,397,072 B (+24.3%) |

Build time, from `--timings` on the plugin-enabled build: the `mlua-sys` build
script (which compiles vendored Luau) is **60.3 s** of unit time and the `mlua`
crate a further **6.3 s** — against **72.1 s** for the `libsqlite3-sys` build
script the release already pays. The whole-run wall clock is not a clean
comparison on this machine: activating `mlua` changes feature unification for
shared dependencies, so the incremental rebuild recompiled `ratatui`, `tui-term`
and `chrono-tz` as well and took 269 s against the baseline's 294 s from a colder
cache. The per-unit numbers are the honest ones: **about one bundled-SQLite's
worth of C/C++ compilation, added once.**

### Per-target verdicts

Two of the four are verified, two are not, and that distinction is the point of
recording it. Note also that **nothing in this branch has ever run in CI** — the
`plugins` feature is not on `origin/main`, so the existing `--all-features` jobs
(`clippy`, `docs`, `windows-clippy`) have never compiled `mlua` on any runner. No
target may be treated as verified on the strength of a green workflow.

| Target | Toolchain the release uses | Verified here |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` system `g++` | **yes** — the release build above |
| `x86_64-unknown-linux-musl` | `cross`, image `CXX_x86_64_unknown_linux_musl = x86_64-linux-musl-g++` | **yes** — see below |
| `x86_64-pc-windows-msvc` | `windows-latest` MSVC `cl.exe` | **no** — MSVC does not exist off Windows |
| `aarch64-apple-darwin` | `macos-14` Xcode `clang++`, **natively** (the runner is arm64) | **no** — needs macOS and the Apple SDK |

**musl, in detail**, because it was the one target with a real chance of failing:
Rust's musl target links statically, and a C++ dependency there needs a musl
`g++` plus a static `libstdc++.a` — Debian's `musl-tools` ships neither. `cross`'s
image does: its `docker/musl.sh` builds `musl-cross-make` with
`--enable-languages=c,c++,fortran` and its Dockerfile exports
`CXX_x86_64_unknown_linux_musl`. Reproduced locally with an equivalent
`musl-cross` toolchain (`x86_64-linux-musl-g++`, static `libstdc++.a` present),
pointed at through exactly the four variables `cross`'s image sets:

```text
cargo build --release --bins --target x86_64-unknown-linux-musl   → ok, 5m18s
target/x86_64-unknown-linux-musl/release/thurbox: ELF 64-bit LSB pie executable,
  static-pie linked, stripped      (14,528,536 B, Luau strings present)
```

**Residual musl risk, named.** The local toolchain is GCC 11.2.1; `cross`'s image
pins GCC **9.2.0**. Luau asks for `-std=c++17`, which GCC 9 supports, and the
sources compiled here use no C++17 *library* feature GCC 9's libstdc++ lacks —
grepped for the usual gaps and found no `<charconv>`, no `from_chars`, no
`<filesystem>` and no `starts_with` in `Ast/`, `Compiler/`, `VM/` or `Config/`.
That reduces the risk without eliminating it; the resolution is one `cross build`
on a machine with a container engine, which this one does not have. If GCC 9.2
does turn out to be short, the fix is local to `Cross.toml` (a newer image or a
target-specific `CXX`), not to this change.

**Windows and macOS, honestly.** Neither is verifiable here. What can be said:
Luau's upstream builds with MSVC and Apple `clang++` as first-class configurations
and `luau0-src` special-cases neither, and the macOS leg is a *native* build on an
arm64 runner rather than a cross-compile — so it is the same shape as the verified
`linux-gnu` leg, with a different system compiler. As the nearest available proxy
for the Windows target, a local `x86_64-pc-windows-gnu` cross-build was run with
mingw's `x86_64-w64-mingw32-g++` and **succeeded** (4m50s; `thurbox.exe`
11,272,704 B, Luau strings present), which shows the sources and the link survive a
Windows ABI — and says nothing about `cl.exe`, whose C++ dialect and standard
library are the part that is untested. The unqualified statement is: **these
two targets are first proven by the release build.** The mitigation available
without a release is to run `cd.yml`'s build job by `workflow_dispatch` on a
throwaway tag before cutting the real one — which is not this change's to do, but
is what the risk is worth.

## Decisions

### D1: Replace release invariant 2 rather than delete it

The check's own header said Stage C deletes it "rather than switching it off —
there is intentionally no switch", and that instruction is right about the
mechanism and wrong about the property. What made it obsolete is not that the
constraint stopped mattering but that it **reversed**: the release must now carry
the runtime, because a handed-over pane is drawn by it. Deleting would have
dropped a live property; keeping would have been worse than either, because after
the flip no release job asks for the feature *and every release binary contains
it*, so the check would report `ok` about exactly what it claimed to forbid. A
check that has stopped tracking its own property is the one failure mode a green
check cannot survive.

So: `REMOVED` with a **Reason** and a **Migration**, plus an `ADDED` requirement
carrying the property in the new direction. The linter's function is renamed
(`invariant_no_plugin_feature` → `invariant_keeps_plugin_runtime`) so a reader
grepping for the old name lands on the diff that changed it.

**Rejected.** *Delete invariant 2 and add nothing* — the release build would then
have no property at all, and `--no-default-features` slipping into `cd.yml` is a
plausible edit (it is exactly what someone does when a cross-build fails at
release time). *Keep the old check as well, for Stage C's benefit* — two
invariants pointing opposite ways, one of them vacuous. *Make it configurable by
stage* — a flag inside a lint whose entire value is that it is not negotiable.

### D2: The new invariant rejects the flags that drop the runtime, not the ones that ask for it

Two patterns: `--no-default-features` anywhere on a command line, and
`default = [` (a TOML default-feature assignment, i.e. a manifest edit). An
explicit `--features plugins` or `--all-features` is deliberately **allowed** —
after the flip both merely re-request what the default already carries, and
failing them would be a rule about tidiness dressed as a safety property.

`default = [` cannot collide with YAML's own `default:` keys (`cd.yml` has one,
under `workflow_dispatch.inputs`) because the pattern requires `=`; the bats
suite covers the case both ways.

**Rejected.** *Check the composition of the built artifact instead of the
workflow text* — stronger, and it belongs in CI over `cargo tree` (where it now
is) rather than in a workflow linter whose whole premise is that it needs no
toolchain. *Parse the workflow's cargo invocations structurally* — invariant 1
does that for the `on:` block because "and nothing else" is a claim about a whole
mapping; here the claim is "this token does not appear", which grep answers
exactly.

### D3: The CI job keeps its id and changes what it verifies

`all-checks.needs` names `plugins`, and that gate is what branch protection
requires; renaming the job to match its new content would churn the required-check
list for no behavioural gain. Its `name:` changes, its comment explains what it
now covers, and the job id stays.

What it covers changed completely, because every other Rust job now compiles the
host:

- the **1.88 MSRV floor** — every other job runs `stable`, which would keep
  passing long after the declared floor was wrong;
- **`--no-default-features`**, clippy and tests. This is the configuration nothing
  else builds now, and 147 `cfg(feature = "plugins")` sites would rot in silence
  without it. It is also the documented answer for a platform whose toolchain
  cannot build the vendored C++, so it has to keep working;
- the **Luau type-check**, which no Rust job does;
- the **inverted dependency-tree assertion**.

**Rejected.** *Delete the job* — three of those four checks would then exist
nowhere. *Keep testing `--features plugins` explicitly* — that is the default
build, already covered by `nextest`, `clippy`, `check`, `docs`, `windows-*` and
`acceptance`; re-running it here would spend a job to learn nothing. *Add a
separate MSRV job and a separate fallback job* — two more heavy Rust legs on every
PR for a split with no diagnostic value; the job already fails with the command
that broke.

### D4: The bundled example pane is seeded hidden

`PaneDecl::default_visible` serde-defaults to `true`, documented as "what an
author expects" — correct for a plugin someone installed on purpose. `hello`
omits the key, which was invisible while no installed binary ran a plugin at all.
In a default build it opens a demo pane in every fresh install's right column,
which is a user-visible regression shipped by a manifest omission.

The fix is one line, and the rule is held for the **whole bundled set**
(`tests/bundled_manifests.rs`) rather than for the manifest that happened to
remember it — the other five already say `default_visible = false`, each with the
same reasoning in a comment, which is precisely the pattern a checked rule
replaces. The test carries an allowlist,
`PANES_DRAWN_IN_A_NATIVE_PANES_PLACE`, empty today: the first handover adds its
pane there in the same commit that stops `src/app/view.rs` drawing the native one,
so "visible" and "no longer duplicated" cannot land apart.

The test reads the manifests from the source tree and parses them with
`session::plugin_manifest::PluginManifest::from_toml` — which is in `session`, not
behind the feature — so it runs and means the same thing in both configurations,
mirroring `tests/teardown_gate.rs`. Reading the directory rather than
`plugin::discovery`'s table also means a *new* bundled plugin is covered the
moment its directory exists.

**Rejected.** *Flip the serde default to `false`* — it would make every
third-party plugin ship invisible, so an author's pane would need a line to appear
at all; the default is right and the bundled set is the exception. *Leave `hello`
visible as a discoverability aid* — a pane a user did not ask for, in the column
the file viewer and tasks pane want, is the definition of what Stage B promised
not to do. *Drop `hello` from the bundled set* — it is the worked example a plugin
author copies and the end-to-end proof the contract works; hidden costs nothing.

### D5: No runtime `[features] plugins` flag

The earlier prose design set describes Stage B as "Cargo default on, runtime
`[features] plugins` still `false`". There is no such setting in the tree, and
this change does not add one. With every bundled pane seeded hidden the host is
additive: discovery walks a user plugin directory that usually does not exist,
creates it if absent-and-needed only, and starts no VM until a plugin is found.
A switch whose only effect is to skip work that already costs nothing is a
settings row that has to be documented and explained.

That also keeps the spec'd startup budget honest rather than satisfied by
avoidance: `plugin-host/runtime` requires `first_frame_ms` within 100% of a
feature-compiled-out build *while the host boots*, which is the number that now
describes every user's launch.

**Rejected.** *Add the flag for symmetry with the other `[features]`* — those gate
panes a user can see; this would gate nothing observable. *Add it as an escape
hatch if the host misbehaves* — the escape hatch that works is
`--no-default-features` (no VM in the binary at all) and, per plugin, not
installing it; a boolean that leaves the code linked in buys no safety the
capability model does not already give.

### D6: The teardown gate's probe is untouched; the test that pinned the old answer is not

`plugin_host_reaches_the_installed_build` reads `Cargo.toml`'s default feature
list. That is exactly the fact this change edits, so the probe needs no edit — it
now returns `true`, and `recorded_verdicts_match_the_tree` still passes because
every pane row remains blocked by condition 2 (`src/app/view.rs` names all seven
native renderers).

One test asserted the old answer directly:
`a_pane_drawn_only_by_a_gated_build_is_not_handed_over` opened with
`assert!(!plugin_host_reaches_the_installed_build(&root))` and its own message
said "if that changed, this whole row is a release decision to revisit rather than
a probe to update". This is that revisit. It becomes
`the_build_condition_holds_and_still_gates_a_handover`, asserting

1. the condition **holds** — so removing the runtime from `default` fails here,
   with the argument attached, rather than silently emptying every handed-over
   pane;
2. the pure rule is unchanged — `pane_is_handed_over(true, false, false)` is still
   `false`, which is the statement the tree can no longer exhibit; and
3. each pane row is now blocked by *its own* reason, asserted row by row via
   `view_draws_native_pane`, replacing the loop that asserted the build condition
   blocked them all.

**Rejected.** *Delete the test* — it is the only place the build condition is
asserted against the tree in the satisfied direction, and its value is now
forward-looking. *Adjust the probe to keep the test passing* — that is the
failure the brief for this work names explicitly: the probe is the honest reading
of the manifest, and a probe bent to preserve a stale assertion is how a gate
stops meaning anything. *Leave the assertion negated and mark the test ignored* —
an ignored test is a deleted test with a comment.

### D7: The MSRV bump's lint fallout is fixed, not silenced

Raising `clippy.toml`'s `msrv` to 1.88 un-suppresses
`clippy::manual_is_multiple_of` — `u64::is_multiple_of` stabilised in 1.87, so the
lint was msrv-gated off at 1.86 and fires at 1.88. Eight `% N == 0` tick-cadence
checks fail `-D warnings`: six in `src/app/mod.rs` (the automation/task refresh
cadence, the two perf-window gates, the metrics, git and config-reload cadences,
the hook version check) and one negated in `src/app/automation.rs`. Rewritten as
`is_multiple_of`, which is behaviour-identical for the unsigned counters involved.

This was found by running the verification, not by predicting it, and it is the
only reason the change touches `src/` logic. Worth recording because it is the
generalisable part: **an MSRV rise is a lint change**, and the next floor bump
should expect the same class of fallout.

**Rejected.** *`#[allow]` the lint at the eight sites* — eight annotations to
preserve a spelling nobody prefers. *Add it to a clippy `allow` list in
`Cargo.toml`* — the same, once, and it would hide the lint for code written later.
*Leave `msrv` at 1.86 while `rust-version` says 1.88* — the two would disagree,
and `clippy.toml`'s comment says it mirrors the manifest; a deliberately stale
mirror is the failure the comment exists to prevent.

## Module ownership and the architecture rules

Nothing new under `src/` except a manifest line, so no module gains an edge.
`tests/bundled_manifests.rs` is an integration test and therefore outside the
library's module graph, which `tests/architecture_rules.rs` governs — like
`tests/teardown_gate.rs` and the `tests/bundled_*.rs` family, it needs no
allowlist entry. It uses `thurbox::session::plugin_manifest` only, so even read as
a module it would sit inside the allowed direction.
