# Tasks — the workflow-invariant lint

## 1. The check

- [x] 1.1 New `scripts/dev/lint-workflows.sh`: `set -euo pipefail`, optional
      workflow-directory argument defaulting to `<root>/.github/workflows`, a
      `fail` helper that records the file/invariant/line, and a final exit on any
      recorded failure.
- [x] 1.2 Invariant 1 — an awk indentation walk over `cd.yml`'s `on:` block that
      extracts the `push:` mapping's keys and its `branches` list (flow and block
      forms), failing on an extra key, an extra branch, a branch other than
      `main`, or a missing `push:` trigger.
- [x] 1.3 Invariant 2 — scan `cd.yml` (whole-line comments stripped) for
      `--features plugins`, `--features=plugins`, `--all-features`, and a
      `features` list containing `plugins`; carry the comment saying Stage C
      deletes this check rather than switching it off.
- [x] 1.4 Invariant 3 — when `nightly.yml` exists, reject a `publish-*` job id,
      the four channel secrets, and the channel tooling; otherwise report `not
      applicable`.
- [x] 1.5 Invariant 4 — when `nightly.yml` exists, require `prerelease: true` on
      every release-action step and `--prerelease` on every `gh release create`;
      otherwise report `not applicable`.
- [x] 1.6 Fail when `cd.yml` itself is missing, rather than passing.

**Verify:** `shellcheck scripts/dev/lint-workflows.sh` and
`./scripts/dev/lint-workflows.sh` against the committed tree

## 2. Fixture tests

- [x] 2.1 New `scripts/dev/lint-workflows.bats`: a helper that writes a minimal
      `cd.yml` into a temp directory, plus one test per passing case (committed
      tree, block-list form, comment mentioning the feature, absent
      `nightly.yml`).
- [x] 2.2 One test per invariant-1 violation: second branch, `tags:` key under
      `push`, missing `push:` trigger, branch other than `main`.
- [x] 2.3 One test per invariant-2 violation: `--features plugins`,
      `--features=plugins`, `--all-features`, `features = ["plugins"]`.
- [x] 2.4 One test per invariant-3 violation: `publish-chocolatey` job id, each
      of the four secrets, `choco push`, `wingetcreate`.
- [x] 2.5 One test per invariant-4 violation: `prerelease: false`, the key
      omitted, `gh release create` without `--prerelease`; and the passing
      `prerelease: true` case.
- [x] 2.6 A test that a missing `cd.yml` fails.
- [x] 2.7 A test that the real `.github/workflows/` passes, so the committed tree
      is asserted by the same code path CI runs.

**Verify:** `bats scripts/dev/lint-workflows.bats`

## 3. Wiring

- [x] 3.1 `justfile`: add `./scripts/dev/lint-workflows.sh` to the `lint` recipe,
      beside `lint-luau.sh`.
- [x] 3.2 `.github/workflows/ci.yml`: a `workflows` output on the `changes` job
      filtering `.github/**`, `scripts/dev/lint-workflows.sh`,
      `scripts/dev/lint-workflows.bats` and `justfile`.
- [x] 3.3 `.github/workflows/ci.yml`: a `workflow-lint` job gated on that filter
      that installs bats, runs the fixture suite, then runs the lint against the
      tree.
- [x] 3.4 `.github/workflows/ci.yml`: add `workflow-lint` to `all-checks`'s
      `needs`, so it gates rather than advises.

**Verify:** `./scripts/dev/lint-workflows.sh` after the edit (the job's own
`.github/**` edit must not violate an invariant) and `just --list`

## 4. Docs

- [x] 4.1 `CLAUDE.md`: name the check under the enforcement/lint commands, with
      the four invariants in one line each and the Stage C disposal note.

**Verify:** `rumdl check .`

## 5. Full verification

- [x] 5.1 `bats scripts/dev/lint-workflows.bats` and `bats scripts/install.bats`
- [x] 5.2 `git ls-files -z '*.sh' | xargs -0 shellcheck`
- [x] 5.3 `rumdl check .`
- [x] 5.4 `cargo fmt --all -- --check`; `cargo clippy --all-targets --features
      plugins -- -D warnings`; `cargo clippy --all-targets -- -D warnings`;
      `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`;
      `cargo nextest run --all` and `--all --features plugins` — unchanged, since
      no Rust is touched.
