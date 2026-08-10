# release/installer-version Specification

## Purpose
Which version `scripts/install.sh` and `scripts/install.ps1` resolve to when the
user pins none: only a stable `v<major>.<minor>.<patch>` release, never a
prerelease, on every automatic path — so a nightly published as a GitHub
prerelease can never be installed by someone who asked for stable. An explicitly
pinned version is out of scope: it is passed through unfiltered, because pinning
a nightly is the supported way to install one deliberately.
## Requirements
### Requirement: An unpinned install resolves only a stable release tag

When no version is pinned, each installer SHALL resolve a version that matches
`v<major>.<minor>.<patch>` exactly — three dot-separated runs of digits after a
literal `v`, with nothing before and nothing after. A candidate carrying a
prerelease suffix, build metadata, a fourth component, or any non-digit outside
the separators MUST be refused rather than accepted, truncated to its numeric
prefix, or otherwise coerced into a version. The rule MUST apply to every
automatic resolution path an installer has, including the GitHub API answer, so
the property does not depend on what that endpoint filters out.

#### Scenario: A prerelease tag is refused

- **WHEN** the stable-tag rule is asked about `v2.0.0-nightly.20260808`
- **THEN** it answers no

#### Scenario: A plain three-part version is accepted

- **WHEN** the stable-tag rule is asked about `v1.4.12`
- **THEN** it answers yes

#### Scenario: A truncation of a prerelease is not accepted in its place

- **WHEN** the only candidate available is `v2.0.0-nightly.20260808`
- **THEN** no version resolves, and `v2.0.0` is not produced from it

#### Scenario: A near-miss shape is refused

- **WHEN** the stable-tag rule is asked about `v1.2`, `v1.2.3.4`, `1.2.3`, or
  `v1.2.3+build5`
- **THEN** it answers no for each

### Requirement: The releases-page fallback skips prereleases and takes the newest stable

When the API path yields no acceptable version, each installer SHALL scrape the
releases page and select the **first candidate in page order that satisfies the
stable-tag rule**, relying on GitHub rendering newest-first. A prerelease listed
above a stable release MUST be skipped rather than selected or truncated, and
the selection MUST be reachable as a unit that a test can call with a page
fixture instead of a network response.

#### Scenario: A prerelease listed above a stable release

- **WHEN** the page fixture links `releases/tag/v2.0.0-nightly.20260808` first
  and `releases/tag/v1.4.12` second
- **THEN** the selector returns `v1.4.12`

#### Scenario: Several prereleases above the newest stable

- **WHEN** the page fixture links two nightly tags and a release-candidate tag
  before `releases/tag/v1.4.12`
- **THEN** the selector returns `v1.4.12`

#### Scenario: The newest entry is stable

- **WHEN** the page fixture links `releases/tag/v1.5.0` before
  `releases/tag/v1.4.12`
- **THEN** the selector returns `v1.5.0`

#### Scenario: A page with no stable release

- **WHEN** the page fixture links only prerelease tags
- **THEN** the selector returns nothing and resolution fails with the error that
  names how to pin a version, rather than installing a prerelease

### Requirement: An explicitly pinned version is passed through unchecked

An installer SHALL use a version the user pinned exactly as given, without
applying the stable-tag rule to it. Pinning a prerelease is how a nightly is
installed deliberately, so the guard MUST constrain resolution only.

#### Scenario: A pinned prerelease installs

- **WHEN** the user pins `v2.0.0-nightly.20260808`
- **THEN** that exact string is the version used, and no stable-tag check rejects
  it

#### Scenario: A pinned stable version needs no network

- **WHEN** the user pins `v1.4.12`
- **THEN** that string is returned without the API or the page being consulted

