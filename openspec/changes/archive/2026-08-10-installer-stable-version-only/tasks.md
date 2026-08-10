# Tasks — stable-only version resolution

## 1. `install.sh`

- [x] 1.1 `scripts/install.sh`: add `STABLE_TAG_RE`, `is_stable_version` and
      `select_stable_tag` above `get_version`, each with the one-line reason the
      guard exists.
- [x] 1.2 `scripts/install.sh`: `get_version` returns a pinned `VERSION`
      untouched, accepts the API `tag_name` only when `is_stable_version`, and
      pipes the scraped page through `select_stable_tag`.

**Verify:** `sh -n scripts/install.sh` and `shellcheck scripts/install.sh`

## 2. `install.ps1`

- [x] 2.1 `scripts/install.ps1`: add pure `Get-StableTag -PageContent`, matching
      every `releases/tag/<tag>` candidate and returning the first that fully
      matches `^v\d+\.\d+\.\d+$`, else `$null`.
- [x] 2.2 `scripts/install.ps1`: `Get-LatestVersion` returns a pinned `$Version`
      untouched, accepts `tag_name` only when it fully matches the same pattern,
      and delegates the scrape to `Get-StableTag`.
- [x] 2.3 Keep the file ASCII-only and BOM-free (its own suite asserts both).

**Verify:** `pwsh -c '[System.Management.Automation.Language.Parser]::ParseFile(
"scripts/install.ps1", [ref]$null, [ref]$e); $e'`

## 3. Regression tests — bats

- [x] 3.1 `scripts/install.bats`: `is_stable_version` accepts `v1.4.12` and
      refuses `v2.0.0-nightly.20260808`, `v1.2`, `v1.2.3.4`, `1.2.3`,
      `v1.2.3+build5`.
- [x] 3.2 `scripts/install.bats`: `select_stable_tag` against a releases-page
      fixture whose newest entry is a prerelease returns the stable tag below it;
      a fixture whose newest entry is stable returns that; a prerelease-only
      fixture returns nothing.
- [x] 3.3 `scripts/install.bats`: `get_version` with `fetch_url` stubbed to
      return a rate-limit body then the prerelease-first page resolves the stable
      tag — the wiring, not just the selector.
- [x] 3.4 `scripts/install.bats`: `get_version` with `VERSION` set to a
      prerelease returns it verbatim and consults no fetch.
- [x] 3.5 `scripts/install.bats`: the existing under-250-lines test still passes.

**Verify:** `bats scripts/install.bats`

## 4. Regression tests — Pester

- [x] 4.1 `scripts/install.Tests.ps1`: add `Get-StableTag` to the
      defines-the-function table.
- [x] 4.2 `scripts/install.Tests.ps1`: a `Describe 'Get-StableTag'` block with
      the prerelease-first, stable-first, several-prereleases, prerelease-only
      and no-tags fixtures.

**Verify:** `Invoke-Pester -Path scripts/install.Tests.ps1`

## 5. Full verification

- [x] 5.1 `bats scripts/install.bats`
- [x] 5.2 `git ls-files -z '*.sh' | xargs -0 shellcheck`
- [x] 5.3 `rumdl check .`
- [x] 5.4 `cargo fmt --all -- --check`; `cargo clippy --all-targets --features
      plugins -- -D warnings`; `cargo clippy --all-targets -- -D warnings`;
      `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all` and
      `--all --features plugins` — unchanged, since no Rust is touched.
