# Thurbox v2 — Delivery Model, Release Channels, and the Website

v2 is a long refactor that will be unstable for months while v1 keeps shipping.
This document defines how the two coexist: what carries the in-flight work, how
nightly builds reach people who want them, and how the website serves both
versions without confusing anyone on stable.

Two constraints govern everything below, and they pull in opposite directions:

> **C1 — A v1 user must not be able to end up on v2 by accident.** Not through
> `install.sh`, not through Homebrew, Chocolatey, winget or the AUR, and not by
> reading the docs site.

> **C2 — v1 must stay exactly as maintainable during the refactor as it is
> today.** No v1 fix may become slower or riskier to ship because v2 is in
> flight.

The obvious answer — a long-lived `v2` branch — satisfies C1 and violates C2.
§1 shows the cost; §2 buys C1 a cheaper way.

---

## 1. Why not a long-lived `v2` branch

An earlier draft of this document specified a `v2` branch merged back at 2.0.0,
with `main` merged in at every v1 release. It is the instinctive answer and it
is the expensive one.

| Cost | Magnitude |
|---|---|
| **Divergence** | `main` runs ~3.5 commits/day (50 in the 14 days to 2026-08-08). A 4–6 month refactor is 400–700 commits to reconcile |
| **Collision** | v2's work *is* the restructuring of `src/app/mod.rs` (14,605 lines) and `src/ui/` (36 modules) — precisely the files v1 fixes touch. Every merge conflicts inside the file being rewritten |
| **Plumbing** | `ci.yml` must be widened to the branch; a second release workflow must live on it and drift from `cd.yml`; `pages.yml` must do a cross-branch checkout, a mechanism the earlier draft itself flagged as possibly fragile and gave a fallback for |
| **The cut** | One merge commit is the single riskiest moment in the project, and it lands with no incremental user exposure behind it |
| **Abandonment** | If v2 stalls at Phase 2, the work is orphaned on a branch nobody wants to merge |

The branch buys exactly one property: `main` cannot accidentally ship v2. That
property is worth having, and §2 buys it for a Cargo feature flag.

The branch model is not deleted from the record — it is the documented fallback,
with its adoption trigger in [§10](#10-fallback-the-branch-model).

---

## 2. The delivery model

**Every phase of v2 lands on `main`, continuously, behind a Cargo feature.**

```toml
# Cargo.toml
[features]
default = []
plugins = ["dep:…"]   # the v2 plugin host, runtime supervisor, and view-tree renderer
```

Every v2 module is `#[cfg(feature = "plugins")]` at its root. `cd.yml` builds
stable **without** the feature, so the released v1 binary does not contain the
plugin host in a disabled state — it does not contain it at all. The nightly
workflow builds **with** it.

| Property | How it is obtained |
|---|---|
| Stable can never ship v2 (C1) | The stable build does not compile the v2 modules |
| v1 velocity is untouched (C2) | There is no second branch, so there is nothing to merge |
| No CI widening | Every PR targets `main` and runs the existing required suite |
| Both configurations stay green | CI adds one matrix leg building and testing `--features plugins` |
| Abandonment is cheap | Deleting a Cargo feature is an ordinary PR |
| Exposure is incremental | The gate's default can move at a stage boundary (§3) without a version cut |
| No artifact-size regression at all | The Luau VM is kilobytes of linked code, not a vendored runtime ([ADR-V3](ARCHITECTURE.md#adr-v3)) |

### Costs, accepted explicitly

- **Two configurations.** `cargo clippy` and `cargo nextest` run twice in CI.
  This is the direct price of C2 and it is paid in machine time, not in
  contributor attention.
- **Coexisting implementations.** A migrated pane exists twice — the native
  Rust pane and its plugin — from the moment the plugin lands until Stage C.
  §5 covers why that is cheaper than it sounds, and how it ends.
- **`cfg` branching at dispatch sites.** Each migrated pane adds one branch in
  `App::view`, removed with the native pane.

### What this does *not* change

The v1 release process is byte-identical to today: `cd.yml`'s push trigger names
`main` and nothing else, `cog bump --auto` reads the same history, and the four
platform artifacts are built from a feature set that has not moved. A v1 fix in
the middle of Phase 2 is the same PR it would have been before v2 started.

---

## 3. Three stages, three gate positions

This is the part that most reduces risk, and it follows from one observation:

> **Almost all of v2 is additive.** The plugin host, the VM, the
> view-tree renderer, the `@thurbox` modules, the command registry, the CLI
> verbs — none of it removes v1 behavior. Only the final pane swaps and the
> extension teardown do.

So the **value** of v2 (anyone can write a pane) does not have to wait for the
**breakage** of v2 (native panes deleted, `[features]` flags replaced,
`extensions/` removed). Splitting them is free, and it moves the interesting
half months earlier onto the stable channel.

| Stage | Cargo `plugins` | Runtime `[features] plugins` | Ships as |
|---|---|---|---|---|
| **A — Development** | off by default | — | nightly only |
| **B — Experimental** | **on** by default | off by default | an ordinary v1 **minor** release |
| **C — Default** | on | **on** by default | **2.0.0** |

### Stage A — development

The gate is off in stable builds; only nightly compiles the feature. Phases 0
through 5 of [MIGRATION.md](MIGRATION.md) run here — the whole plugin host, the
service half, motion, the bundled plugins, and the command registry. Nothing
about v2 is reachable by a stable user, and nothing about v1 is harder to ship.

Phase 0 is the exception that proves the model: it is ungated, because it is
behavior-preserving and snapshot-identical, so it ships to v1 users immediately
and gets exercised by them before anything depends on it.

### Stage B — experimental, on the stable channel

Once the plugin host is stable enough to write against, the Cargo feature flips
to **default on** and thurbox ships it in a normal v1 minor release. The runtime
setting `[features] plugins` still defaults to `false`, so:

- A user who does nothing gets v1, unchanged, with every native pane intact.
- A user who sets `plugins = true` and points `THURBOX_PLUGIN_RUNTIME` at their
  gets the plugin host, additively, alongside the native panes.

Three things fall out, and each is worth more than it costs:

1. **The API gets real users before it is frozen.**
   [N4](CONSTITUTION-DELTA.md#n4--the-plugin-api-is-additive-within-a-major-version)
   makes the protocol additive-only within a major version. Freezing it at 2.0.0
   having only ever been used by its authors is how plugin APIs acquire
   permanent mistakes. Stage B is the period in which those mistakes are still
   free to fix.
2. **Artifact size is a non-issue.** [ADR-V3](ARCHITECTURE.md#adr-v3) links the
   Luau VM into the binary, which costs a few hundred kilobytes — not the
   40–90 MB per archive a bundled JavaScript runtime would have. There is no
   staged rollout to design around it and no packaging channel (`packaging/`:
   brew, AUR, Chocolatey, winget) that needs changing. The single-binary install
   survives Stage C untouched.
3. **The remaining runtime question is a build question, and CI answers it.**
   "Does `mlua`'s vendored Luau cross-build for Windows ARM64, musl, and the
   macOS universal build?" is settled by a green matrix in Phase 0, not by
   months of user reports — because nothing has to be *found* on the user's
   machine at runtime ([MIGRATION §open questions](MIGRATION.md#9-open-questions)).

**Exit criterion for Stage B**: the protocol has gone one full minor release
without a non-additive change, and at least one plugin exists that thurbox did
not write.

### Stage C — default, and 2.0.0

The runtime setting defaults to `true`, the runtime is bundled, the native panes
are deleted, `[features]` flags are replaced by `thurbox plugin enable|disable`,
and the v1 extension system is removed. This is the breaking release, and by the
time it happens every part of it except the deletions has been running on the
stable channel for months.

### Cutting the stage transitions by hand

Each stage transition is a flag flip, and a flag flip is not something the
release train can be trusted to price correctly. The train reads commit *types*:
flipping `plugins` from off to on by default is one line of `Cargo.toml`, which
`cog bump --auto` will read as whatever the commit was labelled — and which the
artifact-relevance gate will happily wave through, because `Cargo.toml` is on
its path list. The version that ships a stage transition should be chosen, not
computed.

`cd.yml` already has the mechanism: a `workflow_dispatch` with a `version`
input, which runs `cog bump --version <v>` instead of `--auto` and **skips the
artifact-relevance gate** — an explicit human cut is always honoured. It is how
v1 crossed 0.x → 1.0.0, and it carries over unchanged.

| Transition | How it is cut | Why |
|---|---|---|
| Any ordinary v1 release, during any stage | The train — push to `main` | Unchanged from today. This is the common case and stays automatic |
| **A → B** (`plugins` on by default in Cargo) | Dispatch with an explicit minor, e.g. `1.9.0` | The train would compute *a* version from the flip commit's type; which minor carries the plugin host to stable users is a decision, and it is announced |
| **B → C** (2.0.0) | Dispatch with `2.0.0` | The breaking release. `cog bump --auto` reaches a major from 1.x only via a `BREAKING CHANGE` footer, which makes the biggest release in the project's history contingent on a commit-message convention. Dispatch it |
| A hotfix on a stage-transition release | The train | Ordinary `fix`, ordinary patch |

Two properties of the dispatch path matter here and are easy to lose:

1. **A forced cut skips the relevance gate, not the invariants.** §9's rules
   still bind — in particular, a dispatched release before Stage C must not
   build `--features plugins`. The gate lives in `cd.yml`'s build matrix, not
   in the trigger, so this holds by construction; the workflow-lint script that
   checks invariants 1–4 should assert it against the dispatch path too, since
   a hand-typed version is exactly the moment someone is also hand-editing the
   workflow.
2. **A dispatch cannot rescue a bad trunk.** It tags whatever `main` is at, and
   the same CI must be green. Manual means *choosing the version*, not
   *bypassing the checks* — if that distinction ever blurs, the manual path has
   become the thing the train exists to prevent.

---

## 4. The nightly channel

People who want to try v2 before Stage B need builds, and those builds must be
unmistakably not stable.

### 4.1 The workflow

A `nightly.yml` on `main`, triggered by `schedule` (daily) and
`workflow_dispatch` (§4.3), which:

- runs only if the day's `main` is CI-green — a red trunk produces no nightly
  rather than a broken artifact;
- builds the same four platform artifacts `cd.yml` builds, **with
  `--features plugins`**;
- publishes a GitHub release with **`prerelease: true`**;
- **runs none of** `publish-homebrew`, `publish-aur`, `publish-chocolatey`,
  `publish-winget`;
- prunes to the most recent 14 nightlies, so the release list stays finite.

The package channels are the sharpest edge. Homebrew and the AUR update
automatically, and Chocolatey and winget go through moderation queues a
prerelease would pollute — the same queues the throttles in `cd.yml` already
exist to protect. None of them should see a v2 artifact before 2.0.0.

Building nightly from `main` rather than from a branch has a property the branch
model could not offer: **a nightly is a stable build plus one Cargo feature.**
Any difference in behavior is attributable to the feature, because nothing else
differs.

### 4.2 Tag naming, and why it is not semver

Nightly tags are **non-semver**: `nightly-2026-08-08`, not
`v2.0.0-nightly.20260808`.

Cocogitto computes the next version from the latest tag reachable from the
branch, and under this model nightly tags are reachable from `main`
immediately — there is no branch separating them. A semver-shaped prerelease tag
is exactly what a version calculator will try to interpret; a tag that does not
parse as semver is skipped instead.

**This must be verified against cocogitto's actual tag filter before the first
nightly ships**, not assumed. It is now a release-blocking check rather than a
background question, because trunk-based delivery makes the tags reachable on
day one instead of at the merge. If `cog` rejects unparseable tags rather than
ignoring them, the fallback is to publish nightlies as releases pointing at a
SHA with no tag at all.

### 4.3 Triggering a nightly by hand

The daily schedule is the floor, not the only path. `workflow_dispatch` on
`nightly.yml` covers the cases the schedule cannot:

- **A plugin author needs today's host**, and the API changed this morning.
  Waiting until tomorrow's build to hand them an artifact is the kind of
  friction that loses the third-party plugin Stage B's exit criterion depends
  on.
- **The scheduled build was skipped** because `main` was red at the time, and
  the fix has since landed.
- **Verifying the nightly pipeline itself** without waiting a day per attempt —
  which matters most at Phase 0, when nothing has run yet.

Two inputs, both optional:

| Input | Default | Effect |
|---|---|---|
| `ref` | `main` | Build a specific branch or SHA |
| `tag_suffix` | none | Disambiguates a second nightly on one day (`nightly-2026-08-08.2`), so a re-run does not collide with the scheduled tag |

**A dispatched nightly obeys every rule the scheduled one does** — `prerelease:
true`, no package-channel job, the 14-build prune, the CI-green guard. The
trigger changes when a build happens, never what it is.

The `ref` input carries one caveat worth stating, because it quietly costs the
property §4.1 is built on: a nightly built from a branch is **no longer "stable
plus one Cargo feature"**, so a behavior difference is no longer attributable to
the feature alone. That is an acceptable trade for handing someone a build of an
unmerged change, and a bad one for anything a bug report will be filed against.
Branch-built nightlies should carry `tag_suffix` naming the branch, so the tag
says what it is.

### 4.4 Installing a nightly

Both installers already accept an explicit version, so no installer feature is
needed:

```bash
VERSION=nightly-2026-08-08 curl -fsSL .../install.sh | sh
THURBOX_VERSION=nightly-2026-08-08 irm .../install.ps1 | iex
```

---

## 5. Why coexistence beats divergence

The one real cost of the trunk model is that a migrated pane exists twice
between the day its plugin lands and Stage C. This is the objection to answer,
because [MIGRATION.md](MIGRATION.md)'s earlier principle was "delete on
completion — no dual implementations".

That principle is **amended**: deletion is gated on the plugin becoming the
*default* for that pane, not on the plugin merely existing.

Four things make the interim cheap:

1. **A frozen implementation costs almost nothing.** The Rust tasks pane is
   finished. It is not being edited during the migration; it is being kept
   compiling. The maintenance burden of code nobody touches is close to zero,
   and it is bounded — it ends at Stage C.
2. **The pair is self-checking.** Constitution rule 10, as amended in
   [CONSTITUTION-DELTA.md](CONSTITUTION-DELTA.md#rule-10--test-driven-development),
   requires a migrated pane to pass the tests its predecessor passed. With both
   present, the same insta snapshot is asserted against both implementations in
   the same run. Under the branch model the native pane's snapshot would be a
   memory of another branch; here it is a live assertion.
3. **Deletion is still per-pane and still one PR.** At Stage C each pane is
   deleted in the PR that flips it to plugin-default, along with its
   `InputFocus`, `Action`, `PanelAreas`, `ClickAction`, and `FeatureFlags`
   entries — the same bisectable, per-pane teardown the branch model wanted, on
   the trunk.
4. **It is an insurance policy the branch model cannot offer.** If one pane's
   migration stalls, 2.0.0 can ship with that pane still native. The failure
   mode degrades to "the code review isn't a plugin yet" instead of blocking the
   release or forcing a bad merge.

---

## 6. CI

Three changes, all on `main`, all small:

| Change | Why |
|---|---|
| A `--features plugins` matrix leg (build, clippy, nextest) | Otherwise the gated code rots and Stage B is a surprise |
| Luau toolchain — `luau-analyze` strict, plugin test runner | Constitution rule 3 as extended in [CONSTITUTION-DELTA.md](CONSTITUTION-DELTA.md). Lands in Phase 0 so the first Luau PR does not also carry the toolchain |
| A workflow-invariant lint (§9) | Invariants 1–3 are one script and are expensive to discover broken |

Notably absent: widening `ci.yml`'s `pull_request` trigger. Under the branch
model that was a prerequisite hazard — a PR into `v2` would have run no checks
at all. With one trunk, the existing `branches: [main]` trigger already covers
every v2 PR. **The trunk model deletes that hazard rather than mitigating it.**

The `check-release` gate in `cd.yml` needs no change: its `shipped` step already
keys on `src/`, `Cargo.toml`, and friends, so v2 work on `main` participates in
v1 releases exactly as any other source change does — which is correct, because
it *is* in the source tree even when it is not in the binary.

---

## 7. The installer hazard

Publishing nightlies as prereleases is safe on the **primary** version-resolution
path and unsafe on **both fallback** paths. Both installers resolve a version in
two steps:

```sh
# Primary — safe. GitHub's releases/latest excludes prereleases and drafts.
fetch_url "https://api.github.com/repos/${REPO}/releases/latest"

# Fallback — scrapes the releases page, which DOES list prereleases.
grep -o 'releases/tag/v[0-9.]*' | head -1
```

`install.sh`'s character class is `[0-9.]`, so a nightly tagged
`v2.0.0-nightly.20260808` would match as `v2.0.0` — a version that does not exist
as a release, failing later at asset download with a confusing 404.
`install.ps1`'s pattern is worse, because it accepts the whole tag:

```powershell
[regex]::Match($page.Content, 'releases/tag/(v[0-9][0-9A-Za-z.\-+]*)')
```

A Windows user whose API call failed would install a v2 nightly believing it to
be stable.

**§4.2 already defuses this.** Both patterns anchor on a literal `v`, and
`nightly-2026-08-08` has no `v`, so neither fallback can match a nightly under
the chosen tag format — the scrape skips it and finds the newest `v1.x.y` on the
page, which is the correct answer. The two sections were written independently
and the interaction went unnoticed; it is stated here so nobody "fixes" the tag
format back to something semver-shaped without realizing it re-arms the bug.

**The fix is still required**, as defense in depth and as the thing that *keeps*
the property true: both fallbacks should accept only `v<major>.<minor>.<patch>`
with no suffix, with a regression test using a prerelease fixture in the existing
`scripts/install.bats` and `scripts/install.Tests.ps1` suites. It is a small,
testable change with CI jobs already gating both files. It is no longer a release
blocker for the first nightly — it is a Phase 0 hardening item.

---

## 8. The website

### 8.1 Shape

| Path | Serves |
|---|---|
| `/` and `/docs/*` | v1 stable — unchanged, and the default for every visitor |
| `/v2/*` | v2 documentation |

Stable stays at the root deliberately. Most visitors want the thing they can
install today, and moving v1 to `/v1/` would break every existing inbound link
and search result for the sake of symmetry.

### 8.2 Build mechanics

The existing `website.11tydata.js` computes permalinks from `filePathStem`, so
`website/v2/installation.html` already emits `/v2/installation.html` with **no
config change**.

Because there is no branch, that is the entire mechanism. The v2 pages live at
`website/v2/` on `main`, next to the code they describe, and `pages.yml` builds
them in the same pass as everything else. The cross-branch checkout the earlier
draft needed — and the fallback it needed for that — are both gone.

`pages.yml` already has `workflow_dispatch` and triggers on `website/**`, so no
trigger change is required either.

### 8.3 Required affordances

Serving two versions is mostly a question of never letting someone read the
wrong one by accident:

| Affordance | Why |
|---|---|
| **Version switcher** in the docs chrome (`base.njk` / `sidebar.njk`) | The only way to move deliberately between trees |
| **Persistent banner** on every `/v2/` page — naming the current stage, and stating that it is not installable via Homebrew/winget/Chocolatey/AUR | A visitor arriving from search has no other signal |
| **`noindex`** on `/v2/*` until 2.0.0 | Otherwise v2 pages compete with v1 in search results, which is precisely the accident C1 forbids |
| **Exclude `/v2/` from `sitemap.njk`** | The sitemap is generated; without this it advertises the v2 tree |
| **No install commands on `/v2/`** that do not carry an explicit `VERSION=` | A copy-pasteable stable-looking command on a v2 page is the easiest way to get this wrong |

The banner's text is stage-dependent, which is the one place the website has to
track §3: at Stage A it reads "unreleased", at Stage B "experimental, opt-in in
v1.x", at Stage C it is removed.

At 2.0.0 the switch is: drop `noindex`, add v2 to the sitemap, move `/v2/` to
`/`, and archive the v1 tree at `/v1/` with canonical tags pointing forward.

### 8.4 Sidebar

`sidebar.njk` hardcodes its section tree in a `{%- set sections = [...] %}`
block. Two trees means either duplicating that block or lifting it into
`website/_data/nav.js` keyed by version. The data-file route is the better one —
it is the same amount of work either way, and it stops the two sidebars
drifting.

---

## 9. Invariants

Things that must remain true for the whole refactor. Each is cheap to check and
expensive to discover broken:

1. `cd.yml`'s push trigger names `main` and nothing else.
2. `cd.yml` never builds with `--features plugins` before Stage C.
3. No package-channel publish job ever runs from `nightly.yml`.
4. Every nightly GitHub release has `prerelease: true`.
5. `install.sh` and `install.ps1`, with no version pinned, resolve to the latest
   **stable** tag — including when the API path fails and the scrape fallback
   runs (§7).
6. `/v2/` is `noindex` and absent from the sitemap until 2.0.0.
7. The `--features plugins` CI leg is required, not advisory. A gated feature
   that is allowed to fail is a gated feature that is broken.
8. A dispatched release is subject to invariants 1–7 exactly as a pushed one
   is. The `version` input chooses a number; it never relaxes a gate.

Invariants 1–4 are enforceable by a small workflow-lint script, in the spirit of
Constitution rule 11. Invariant 5 belongs in `install.bats` and
`install.Tests.ps1` as a regression test with a prerelease fixture. Invariant 7
is a branch-protection setting.

---

## 10. Fallback: the branch model

Trunk-based delivery is the plan. The branch model is the documented retreat, so
that adopting it later is a decision rather than a capitulation.

**Adopt a `v2` branch if any of these fire:**

1. **A v2 PR breaks the stable build twice in one month.** That would mean the
   `cfg` boundary is not holding and the compile-time gate is not buying C1.
2. **The `cfg` surface in `app/mod.rs` exceeds roughly 20 branch sites.** Past
   that, the dispatch layer is being maintained in two shapes at once and the
   coexistence argument in §5 stops holding.
3. **Stage B slips past the point where the trunk carries more v2 than v1.** If
   most of `main` is gated code, the gate has become a branch with worse
   ergonomics.

If adopted, the earlier plan applies in full: widen `ci.yml` to
`pull_request: branches: [main, v2]` **first**, merge `main` → `v2` at every v1
release, never reverse the direction, and cut 2.0.0 with a merge commit rather
than a squash so the per-pane deletions stay bisectable.

---

## 11. Open questions

1. **Does cocogitto ignore or reject non-semver tags?** §4.2 depends on it, and
   under trunk-based delivery nightly tags are reachable from `main`
   immediately. Verify before the first nightly.
2. **Where exactly does the Cargo feature boundary fall?** Some Phase 0 work
   (the view-tree types in `session::view`, the native renderer in `ui/`) is
   behavior-preserving and belongs in stable ungated; the host and runtime are
   clearly gated. The renderer is the ambiguous one, and the answer should be
   "ungated" if its snapshots are byte-identical.
3. **Does the demo recording pipeline need a v2 variant?**
   `scripts/demo/record.sh` drives the real TUI, and v2's panes will diverge
   visually before 2.0.0. The website's hero media is currently shared between
   both trees.
4. **Docs-site search**, if added later, must be scoped per version — one index
   across both trees reintroduces exactly the confusion §8.3 prevents.
