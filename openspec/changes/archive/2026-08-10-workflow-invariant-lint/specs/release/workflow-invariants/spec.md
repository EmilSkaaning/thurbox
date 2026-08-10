## ADDED Requirements

### Requirement: A deterministic check gates the release-delivery invariants

A script SHALL verify the release-delivery invariants over the tracked workflow
files and exit non-zero when any is violated, naming the file, the invariant, and
the offending line. It MUST require no tool beyond those the repository's other
shell checks already require — no YAML library and no Python — and it MUST be
runnable both from `just lint` and from a required CI job.

The check SHALL accept the workflow directory to inspect as an optional argument,
defaulting to the repository's own, so that every violating case in this
capability is reachable by pointing it at a fixture directory rather than by
committing a broken workflow.

#### Scenario: A clean tree passes

- **WHEN** the check runs against the workflow files as committed
- **THEN** it exits zero and reports each invariant it verified

#### Scenario: A fixture directory is inspected instead of the repository's

- **WHEN** the check is given a directory containing fixture workflow files
- **THEN** it inspects those files and ignores the repository's own

#### Scenario: A violation names what broke

- **WHEN** any invariant is violated
- **THEN** the check exits non-zero, and its output names the workflow file, the
  invariant, and the line that violates it

#### Scenario: The release workflow is missing

- **WHEN** `.github/workflows/cd.yml` is absent
- **THEN** the check fails rather than passing on the grounds that there is
  nothing to check

### Requirement: The release workflow's push trigger names only the trunk

The check SHALL parse the release workflow's `on:` block structurally and require
that its `push` trigger carries exactly one key, `branches`, whose value is
exactly `main`. Both the flow form (`branches: [main]`) and the block form
(`branches:` followed by `- main`) MUST be accepted. Any additional branch, any
additional key under the same trigger — including `tags` or `paths` — and a
missing `push` trigger MUST each fail, because the invariant is a statement about
the whole trigger rather than about one list.

#### Scenario: The committed trigger passes

- **WHEN** the release workflow's `on.push` is `branches: [main]` and nothing else
- **THEN** the invariant passes

#### Scenario: A second branch is added

- **WHEN** the branch list becomes `[main, v2]`
- **THEN** the check fails, naming the extra branch

#### Scenario: A tag trigger is added beside the branch list

- **WHEN** a `tags:` key is added under the same `push` trigger
- **THEN** the check fails, naming the key that is not `branches`

#### Scenario: The block list form is accepted

- **WHEN** the branch list is written as `branches:` followed by an indented
  `- main`
- **THEN** the invariant passes

#### Scenario: The push trigger is removed

- **WHEN** the release workflow has an `on:` block with no `push` trigger
- **THEN** the check fails rather than treating the absent trigger as satisfying
  the invariant

### Requirement: The release workflow never builds with the plugin feature

The check SHALL reject any request for the plugin feature in the release
workflow: `--features plugins` and `--features=plugins` in either flag order,
`--all-features` (which enables it transitively), and a manifest edit that adds
`plugins` to a `features` list. A line whose content is entirely a YAML comment
MUST be ignored, so the invariant can be documented inside the workflow it
constrains without the documentation tripping the check.

#### Scenario: The committed release workflow passes

- **WHEN** the release workflow contains no plugin-feature request
- **THEN** the invariant passes

#### Scenario: A gated build reaches a release job

- **WHEN** a release job runs a cargo command with `--features plugins`
- **THEN** the check fails, naming the line

#### Scenario: All features enables the gate transitively

- **WHEN** a release job runs a cargo command with `--all-features`
- **THEN** the check fails, because the plugin feature is among them

#### Scenario: A comment mentioning the feature is not a violation

- **WHEN** a line in the release workflow is a comment explaining that
  `--features plugins` must not appear
- **THEN** the invariant still passes

### Requirement: The nightly workflow publishes to no package channel

While a nightly workflow exists, the check SHALL reject any package-channel
publication from it: a job whose id begins with the publish prefix the release
workflow uses for channels, a reference to any of the four channel credentials,
and a use of the channel tooling. When no nightly workflow exists the invariant
MUST pass vacuously, so the check can be landed before the workflow it
constrains.

#### Scenario: No nightly workflow yet

- **WHEN** `.github/workflows/nightly.yml` is absent
- **THEN** the invariant passes and reports that it was not applicable

#### Scenario: A channel publish job is added to the nightly workflow

- **WHEN** the nightly workflow declares a job whose id begins with the channel
  publish prefix
- **THEN** the check fails, naming the job

#### Scenario: A channel credential is referenced from the nightly workflow

- **WHEN** the nightly workflow references the Chocolatey, winget, Homebrew-tap
  or AUR secret
- **THEN** the check fails, naming the secret

#### Scenario: Channel tooling is invoked from the nightly workflow

- **WHEN** the nightly workflow invokes the Chocolatey push, `wingetcreate`, the
  Homebrew tap remote, or the AUR deploy action
- **THEN** the check fails, naming the invocation

### Requirement: Every nightly release is marked as a prerelease

While a nightly workflow exists, every step in it that creates a GitHub release
SHALL declare the release a prerelease: a step using the release action MUST set
`prerelease: true`, and a `gh release create` invocation MUST pass
`--prerelease`. A release-creating step that sets the flag to false, or omits it,
MUST fail — an unmarked nightly becomes `releases/latest`, which every unpinned
installer resolves to.

#### Scenario: No nightly workflow yet

- **WHEN** `.github/workflows/nightly.yml` is absent
- **THEN** the invariant passes and reports that it was not applicable

#### Scenario: A release action step marked prerelease

- **WHEN** the nightly workflow's release-action step sets `prerelease: true`
- **THEN** the invariant passes

#### Scenario: A release action step not marked prerelease

- **WHEN** the nightly workflow's release-action step sets `prerelease: false` or
  omits the key
- **THEN** the check fails, naming the step

#### Scenario: A CLI release creation without the flag

- **WHEN** the nightly workflow runs `gh release create` without `--prerelease`
- **THEN** the check fails, naming the line
