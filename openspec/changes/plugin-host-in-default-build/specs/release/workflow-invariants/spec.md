# release/workflow-invariants Specification

## REMOVED Requirements

### Requirement: The release workflow never builds with the plugin feature

**Reason**: The invariant existed to keep a v1 release binary free of the v2
runtime while the runtime was an optional dependency. Stage B makes the plugin
host part of the crate's default feature set, so the release workflow now ships
it deliberately — and the check's own header said it is deleted in the change
that flips the default rather than switched off.

Keeping it would be worse than dropping it. The check looks for an explicit
plugin-feature *request* on a command line, and after the flip there is none: a
plain `cargo build --release` carries the runtime through the default feature
list. The invariant would therefore keep reporting `ok` about a release binary
that contains exactly what it claims to forbid — a check that has stopped
tracking its own property, which is the failure mode a green check is trusted not
to have.

**Migration**: The property is not abandoned, it is inverted, and the replacement
requirement below carries it in the new direction: the release workflow must not
build *without* the runtime. That is now the hazard, because once a native pane
is handed over to its bundled plugin the runtime is what draws that pane, and a
release built with default features suppressed would ship a binary whose panes
are empty columns — the same silent failure the old invariant was protecting
against, arrived at from the other side.

Nothing else about the check changes: the release workflow's push trigger, the
nightly channel's exclusion from package channels, and the nightly prerelease
marking are untouched, and the fixture-directory argument they are all tested
through is unchanged.

## ADDED Requirements

### Requirement: The release workflow does not build without the plugin runtime

The check SHALL reject any release-workflow build that suppresses the crate's
default features, since the plugin runtime is delivered through them:
`--no-default-features` in any position, and a manifest edit that rewrites the
default feature list. A line whose content is entirely a YAML comment MUST be
ignored, so the invariant can be documented inside the workflow it constrains
without the documentation tripping the check.

The invariant is directional and MUST NOT be read as forbidding an explicit
request for the feature: a release job naming `--features plugins` or
`--all-features` is redundant but harmless, because it asks for the runtime the
release is required to carry.

#### Scenario: The committed release workflow passes

- **WHEN** the release workflow builds with plain `cargo build --release` and
  `cross build --release`
- **THEN** the invariant passes, because the default feature set carries the
  runtime

#### Scenario: A release job suppresses default features

- **WHEN** a release job runs a cargo command with `--no-default-features`
- **THEN** the check fails, naming the line

#### Scenario: A manifest edit rewrites the default feature list

- **WHEN** a release job edits `Cargo.toml` to assign a different `default`
  feature list
- **THEN** the check fails, naming the line

#### Scenario: An explicit feature request is not a violation

- **WHEN** a release job runs a cargo command with `--features plugins` or
  `--all-features`
- **THEN** the invariant passes, because both ask for the runtime rather than
  removing it

#### Scenario: A comment mentioning the flag is not a violation

- **WHEN** a line in the release workflow is a comment explaining that
  `--no-default-features` must not appear
- **THEN** the invariant still passes

#### Scenario: The release workflow is missing

- **WHEN** `.github/workflows/cd.yml` is absent
- **THEN** the check fails rather than passing on the grounds that there is
  nothing to check
