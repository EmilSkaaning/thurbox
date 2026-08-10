# Design — the workflow-invariant lint

## 1. Where the pieces live

No crate module and no Rust type are added, so `tests/architecture_rules.rs` is
untouched: its allowlist governs module edges under `src/`, and this change adds
none. That is worth stating rather than assuming, because it is also the reason
the check is *not* a Rust test.

```text
scripts/dev/lint-workflows.sh    the four invariants, one function each
scripts/dev/lint-workflows.bats  fixture directories, one per violating case
justfile                         `lint` runs it
.github/workflows/ci.yml         `workflow-lint` job + `workflows` path filter
```

`scripts/dev/` is where `lint-luau.sh` lives, and this is the same kind of thing:
a repository-shaped structural check with no dependency beyond the shell, run
from `just lint` and from CI.

## 2. Why a shell script and not a Rust test

`tests/teardown_gate.rs` sets the precedent for encoding a migration invariant as
a Rust test, and it would work here too. Rejected for three reasons:

1. **It would put the check behind a compile.** `cargo nextest` is gated on the
   `rust` path filter; a PR that edits only `.github/workflows/` does not run it.
   Inverting that (making the Rust suite run on workflow edits) makes every
   workflow typo cost a full build.
2. **The subject is YAML, not Rust.** A Rust test would read the file as text and
   do exactly what the shell does, with a compile in front of it.
3. **`just lint` is where a contributor looks.** The lint targets are the shell
   family (`shellcheck`, `rumdl`, `lint-luau.sh`); a fifth member of that family
   needs no explanation.

The cost is that the check is tested by bats rather than by nextest, which is
already an established suite in this repo (`scripts/install.bats`, gated by the
`install-script` CI job).

## 3. Why invariant 1 is parsed and the other three are scanned

Invariant 1 is the only one whose statement is about **structure**. "Names `main`
and nothing else" is a claim about a specific mapping — `on:` → `push:` — and a
grep for `branches: [main]` would pass a file that also carried
`tags: ['v*']` two lines below, which is exactly the violation worth catching. So
the script walks the file by indentation: find the top-level `on:`, find the
second-level `push:`, collect that mapping's keys and its `branches` value, and
require the key set to be exactly `{branches}` and the value list to be exactly
`{main}`.

The other three are claims about **presence or absence of a token anywhere in a
file**, which is what a scan is for. `--all-features` in a release job is a
violation wherever it appears; `prerelease: true` is required in whichever step
creates a release. Parsing those would add a YAML model to reach the same verdict.

The awk indentation walk is deliberately narrow: it understands two-space nesting,
flow sequences, and block sequences, and nothing else. A workflow written with
tabs or with a quoted `"on":` key would not be understood — so the walk **fails
when it cannot find the `push:` trigger** rather than reporting success. A
structural check that silently matches nothing is the failure mode this whole
change exists to prevent, and it must not be reintroduced by the checker itself.

## 4. Why invariants 3 and 4 pass vacuously

`.github/workflows/nightly.yml` does not exist. Two alternatives were considered:

- **Fail until it exists.** Rejected: it asserts that nightlies must ship, which
  is a release-stage decision this change has no standing to make, and it would
  leave `just lint` red on the trunk.
- **Land the check with the workflow.** Rejected, and this is the substantive
  reason for the whole change's timing: a lint written by the person shipping
  `nightly.yml` is a lint shaped to pass that file. Landing it first means the
  first nightly workflow is born gated, and the author discovers the constraint
  from a failing check rather than from a review comment.

The vacuous pass is announced in the output (`not applicable`) rather than silent,
so "the check passed" is never confused with "the file was inspected". The
release-workflow half, by contrast, fails hard if `cd.yml` is missing — that file
exists, and its disappearance is a real event, not a not-yet.

## 5. Why invariant 2 is not configurable

At Stage C, `cd.yml` gains the plugin feature on purpose and invariant 2 becomes
false by design. The options were an environment variable or a marker in `cd.yml`
that switches the check off, versus deleting the check in the PR that flips the
default.

Deletion is chosen. A toggle can be flipped in the same diff that introduces the
thing it was guarding, with no reviewer noticing; removing a named check from a
lint script is a visible line in a diff that has to justify itself. The check
carries a comment saying so, so whoever reaches Stage C knows the intended
disposal rather than reaching for the toggle that is not there.

## 6. Comment handling, and why only for invariant 2

Invariant 2's tokens are the sort of thing a workflow *comment* legitimately
contains — "this must never run `--features plugins`" is a sentence somebody will
write inside `cd.yml`, and a check that punished documenting itself would be
deleted. So invariant 2 strips full-line comments before scanning.

The other scans do not need it and do not get it. A comment naming
`CHOCOLATEY_API_KEY` or `--prerelease` inside `nightly.yml` is not a shape anybody
needs, and every stripped construct is a hole; the narrower the stripping, the
fewer ways a real violation hides behind a `#`. Note that stripping only *whole*
comment lines, not trailing comments, is also deliberate — a trailing comment
cannot hide a violation, because the code before it is still scanned.

## 7. What the bats suite fixtures are

Each violating scenario in the spec is one temp directory holding a minimal
`cd.yml` (and, where relevant, a `nightly.yml`) that is valid enough to exercise
the invariant and nothing more. The script's optional directory argument exists
only for this: without it, testing "a second branch under `push`" would mean
committing a release workflow that releases from two branches.

The suite also runs the script against the **real** `.github/workflows/`, so the
committed tree is asserted clean by the same code path CI runs.
