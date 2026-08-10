# Fail the build when a workflow breaks a release-delivery invariant

## Why

v2 lands on `main` behind a Cargo feature while v1 keeps releasing from the same
trunk. Four properties of the workflow files are what keep those two facts from
colliding, and today **nothing checks any of them**:

1. `cd.yml`'s push trigger names `main` and nothing else. A second branch — or a
   `tags:` clause — under that trigger cuts releases from somewhere nobody
   audited.
2. `cd.yml` never builds with the plugin feature. A `--features plugins` or
   `--all-features` reaching a release build ships the v2 runtime inside a v1
   binary, which is precisely the property the compile-time gate exists to
   guarantee.
3. No package-channel publish job runs from the nightly workflow. Chocolatey and
   winget are **moderated** channels; a nightly reaching either burns a human
   review on a prerelease and, per `CLAUDE.md`, pushes into a queue that starts
   returning 403 when it is fed too fast.
4. Every nightly GitHub release is marked `prerelease: true`. A nightly published
   as a normal release becomes `releases/latest`, and then **every unpinned
   installer on earth resolves to it** — the API path that the companion
   installer hardening treats as the trustworthy one.

Each is a single-line edit away from being false, each is invisible in review
(the diff looks like ordinary CI maintenance), and each is discovered only by a
user who installed the wrong thing. Constitution rule 11 asks that CI decisions
be made by deterministic scripts and notes that CI-config changes need careful
review; a property nobody can check is one that gets reviewed by memory, which
is the part this closes.

Two of the four concern a `nightly.yml` that **does not exist in the tree yet**.
That is the reason to land the check now rather than with the workflow: a lint
written alongside the file it constrains is a lint written by whoever is trying
to ship that file, and it can be shaped to pass. Landing it first means the first
nightly workflow is born gated.

v1 has no equivalent today. `just lint` covers Rust, markdown, shell and Luau;
the workflow files are linted by nothing, and `tests/architecture_rules.rs` —
the repo's other structural allowlist — reads `src/`, not `.github/`.

## What Changes

- **A new `scripts/dev/lint-workflows.sh`**, in the shape of `lint-luau.sh`: no
  dependency beyond the shell tools already required, exits non-zero naming the
  file, the invariant, and the offending line.
- **Invariant 1 is checked structurally, not by grep.** The script walks `cd.yml`'s
  `on:` block by indentation to find the `push:` mapping, and requires that its
  only key is `branches` and that the branch list is exactly `main` — in either
  flow (`[main]`) or block (`- main`) form. A `tags:` or `paths:` key under the
  same trigger fails, because "names `main` and nothing else" is a statement about
  the whole trigger, not about one list.
- **Invariant 2 is a scan of `cd.yml` for the plugin feature**, covering
  `--features plugins`, `--features=plugins`, `--all-features`, and a
  `features = ["plugins"]` manifest edit. It ignores YAML comments, so the
  invariant can be *documented* in `cd.yml` without tripping the check that
  enforces it.
- **Invariants 3 and 4 are checked against `nightly.yml` when it exists and pass
  vacuously when it does not.** A missing file is not a violation, but the check
  is wired in now so the file cannot land ungated. Invariant 3 rejects a job whose
  id begins `publish-`, any of the four channel secrets, and the channel tooling
  (`choco push`, `wingetcreate`, the tap remote, the AUR deploy action).
  Invariant 4 requires every release-creating step to declare the prerelease flag:
  `prerelease: true` in an `action-gh-release` step's `with:`, or `--prerelease`
  on a `gh release create`.
- **Wired into `just lint` and a required CI job**, next to the existing
  `shellcheck` job, gated on a `.github/**` path filter so an unrelated PR does
  not pay for it.
- **The script self-checks its own inputs.** If `cd.yml` is missing, or its `on:`
  block has no `push:` trigger at all, that is a failure rather than a pass — a
  structural check that silently finds nothing to check is the failure mode worth
  guarding against, and it is how invariants 3 and 4 would rot once `nightly.yml`
  is renamed.

## Capabilities

### Added Capabilities

- `release/workflow-invariants`: the release-delivery properties of the workflow
  files that are mechanically enforced, and what each rejects.

## Non-goals

- **Not a YAML validator.** The check tests four named properties. Schema
  validation of GitHub Actions files is a different tool with a different failure
  mode, and `actionlint` is not in the toolchain.
- **Invariants 5–8 are out of scope.** 5 (unpinned installs resolve to stable)
  belongs to `release/installer-version` and its two test suites. 6 (`/v2/` is
  `noindex`) is a website property with no v2 website in the tree. 7 (the
  `--features plugins` leg is required) is a branch-protection setting the
  release strategy assigns to GitHub, not to a script. 8 (a dispatched release
  obeys 1–7) is a consequence of the others rather than a separate check.
- **No enforcement that `nightly.yml` exists.** Whether nightlies ship is a
  release-stage decision; this change only constrains the file's contents once
  somebody writes it.
- **No dependency on `python3` or a YAML library.** `scripts/dev/e2e/lib/` set
  the precedent of doing structured extraction in-shell for exactly this reason,
  and a lint that cannot run because a runner lacks a parser is a lint that gets
  removed.
- **Invariant 2 is not made configurable.** At Stage C, `cd.yml` gains the plugin
  feature deliberately and this invariant is *deleted* — one check removed in the
  PR that flips the default, which is a visible decision. A toggle would let it be
  switched off quietly.

## Impact

- New `scripts/dev/lint-workflows.sh`; it must pass `shellcheck`, which already
  covers every tracked `*.sh`.
- `justfile` — one line in `lint`.
- `.github/workflows/ci.yml` — a `workflow-lint` job, a `workflows` path filter,
  and the job in `all-checks`'s `needs`.
- `CLAUDE.md` — the check named where the other enforcement checks are listed.
- No Rust, no `src/`, no `tests/`.
