# Resolve only stable tags when no version is pinned

## Why

Both installers resolve "the latest version" in two steps, and only the first
step is safe. `scripts/install.sh`:

```sh
response="$(fetch_url "https://api.github.com/repos/${REPO}/releases/latest")"
v="$(echo "$response" | grep -o '"tag_name": *"[^"]*' | head -1 | cut -d'"' -f4)"
[ -n "$v" ] && { echo "$v"; return 0; }

response=$(fetch_url "https://github.com/${REPO}/releases" 2>/dev/null)
v=$(echo "$response" | grep -o 'releases/tag/v[0-9.]*' | head -1 | sed 's|releases/tag/||')
```

GitHub's `releases/latest` endpoint excludes prereleases and drafts, so the
first path is correct. The **fallback scrapes the releases page, which lists
prereleases**, and it takes the first tag it finds — the newest release on the
page, prerelease or not. `fetch_url` uses `curl -s` with no `-f`, so a 404 or a
rate-limited API call returns a body with no `tag_name` and the fallback runs
routinely, not exceptionally.

The two installers then fail differently on the same page:

- `install.sh`'s character class is `[0-9.]`, which stops at the `-`. A release
  tagged `v2.0.0-nightly.20260808` matches as `v2.0.0` — a version that has no
  release, so the install dies later at asset download with a confusing 404.
- `install.ps1`'s pattern is `releases/tag/(v[0-9][0-9A-Za-z.\-+]*)`, which
  accepts the **whole** tag. A Windows user whose API call failed would download
  and install a v2 nightly believing it to be stable.

v2 publishes nightlies as GitHub prereleases, so this stops being hypothetical.
The chosen nightly tag format (`nightly-2026-08-08`) already defuses it by
accident — both patterns anchor on a literal `v`, which that format has none of
— but that is a property of a tag-naming decision recorded in a different
document, and it silently re-arms the moment somebody makes nightly tags
semver-shaped. The correct guard is at the parser, where the requirement is
visible next to the code enforcing it.

This is the installer-hazard hardening item from the v2 release strategy (§7 of
`docs/v2/RELEASE-STRATEGY.md` on the `thurbox-v2` branch, the prose design set
`openspec/config.yaml` names as loose reference), and the regression test its
invariant 5 asks for.

## What Changes

- **Automatic version resolution accepts only `v<major>.<minor>.<patch>`.** No
  suffix, no prerelease, no build metadata. This applies to *both* resolution
  paths in both installers, not only the scrape: a rejected API answer falls
  through to the scrape, which is the correct stable answer, so validating both
  makes the property hold on the installer's own terms rather than depending on
  what GitHub's `releases/latest` filters out.
- **The scrape picks the newest *stable* tag, not the newest tag.** It walks the
  candidates in page order — which GitHub renders newest-first — and takes the
  first one that is a plain three-part version, skipping prereleases instead of
  truncating them into a version that does not exist.
- **An explicitly pinned version is passed through untouched.** `VERSION=…` /
  `-Version …` is a deliberate act, and pinning a nightly to reproduce a bug is
  a thing a user is allowed to do. The guard is on *resolution*, not on the
  argument.
- **The rule lives in one named unit per installer** — a stable-tag predicate in
  `install.sh`, a page-selector function in `install.ps1` — so the pattern is
  written once and both suites can test it directly rather than through the
  network.
- **Prerelease-fixture regression tests** in `scripts/install.bats` and
  `scripts/install.Tests.ps1`: a releases-page fixture whose newest entry is a
  prerelease must resolve to the stable tag below it.

## Capabilities

### Added Capabilities

- `release/installer-version`: what version an installer resolves to when none
  is pinned, which tags it will accept, and which it must refuse.

## Non-goals

- **No change to the tag format.** The nightly naming decision stands; this
  change makes the installers safe regardless of it, which is the point.
- **No ordering logic.** The scrape trusts GitHub's newest-first page order, as
  it does today. Sorting tags by precedence would mean implementing semver
  comparison in POSIX `sh`, to fix an ordering bug nobody has observed.
- **No validation of a pinned version.** An installer that refused
  `VERSION=v2.0.0-nightly.20260808` would remove the only way to install a
  nightly on purpose.
- **No new dependency.** `install.sh` stays POSIX `sh` with `grep`/`sed`;
  `install.ps1` stays regex over the fetched page. Neither gains a JSON or
  semver parser.
- **No change to asset download, checksum verification, or `PATH` handling.**

## Impact

- `scripts/install.sh` — `get_version` plus one predicate and one selector; the
  file must stay under the 250-line ceiling its own test suite asserts.
- `scripts/install.ps1` — `Get-LatestVersion` plus one pure page-selector
  function, exposed for dot-sourced testing like `Get-Target` already is.
- `scripts/install.bats`, `scripts/install.Tests.ps1` — fixture-driven tests.
- No Rust, no CI-config change: the `install-script` and `install-script-ps`
  jobs already gate both files.
