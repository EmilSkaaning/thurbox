# Design — stable-only version resolution

## 1. Where the pieces live

Neither installer is Rust, so `tests/architecture_rules.rs` has nothing to say
about this change: no crate module is added, no module edge is created, and no
new type is introduced. The allowlist is unaffected, which is worth stating
explicitly because every other v2 change so far has had to answer for one.

```text
scripts/install.sh          STABLE_TAG_RE       the pattern, written once
       (POSIX sh)           is_stable_version   predicate over one tag
                            select_stable_tag   page (stdin) -> newest stable
                            get_version         pin -> API -> scrape

scripts/install.ps1         Get-StableTag       page text -> newest stable
       (PowerShell 5.1)     Get-LatestVersion   pin -> API -> Get-StableTag

scripts/install.bats        fixtures + a stubbed fetch_url
scripts/install.Tests.ps1   fixtures against the dot-sourced Get-StableTag
```

The shape is deliberately asymmetric where the languages are. `install.sh` can be
sourced whole (`TEST_TMPDIR` suppresses `main`), so bats can stub `fetch_url` and
drive the real `get_version` end to end — that tests the *wiring*, not only the
pattern. PowerShell's `Get-LatestVersion` calls `Invoke-RestMethod` directly and
Pester's mocking of a cmdlet inside a dot-sourced script is fragile across Pester
versions, so the tag selection is factored into a pure function and tested there,
matching how `Get-Target` and `Get-ExpectedChecksum` are already exposed.

## 2. Rejected alternatives

**Validate only the scrape, leaving the API answer unchecked.** This is the
literal wording of the hardening item, and it is what GitHub's documented
behaviour makes sufficient: `releases/latest` excludes prereleases. Rejected
because it makes the installer's correctness a property of a remote API's
filtering rather than of the installer. Checking both costs one extra call to a
predicate that already exists, and a rejected API answer does not fail — it falls
through to the scrape, which is the right answer anyway. The failure mode of the
stricter version is "does the same thing, one HTTP request later"; the failure
mode of the looser version is "installs a nightly".

**Anchor the scrape pattern with a trailing `"` to force a whole-tag match.** In
the real page every tag URL is inside `href="…"`, so
`releases/tag/v[0-9]+\.[0-9]+\.[0-9]+"` would reject a prerelease by refusing to
match its prefix. Rejected: the pattern would then encode an assumption about
GitHub's HTML quoting, and it would silently stop matching anything the day that
markup changes — resolving *no* version rather than the wrong one, which is
better but still wrong. Extracting candidate tags permissively and then filtering
them with the same predicate the API path uses keeps one statement of what a
stable tag is.

**Sort the candidates by semver precedence instead of trusting page order.**
Rejected: it requires a version comparator in POSIX `sh` for a bug that does not
exist. GitHub renders releases newest-first, today's code already depends on that
(`head -1`), and this change does not widen the dependency — it only skips
entries.

**Reject a pinned prerelease too.** Rejected: it removes the only supported way
to install a nightly, which is the mechanism the nightly channel exists to serve.
A pin is an instruction, not a guess.

**Move the shared pattern into a file both installers read.** Rejected: the whole
point of `install.sh` and `install.ps1` is that each is a single self-contained
file fetched and piped to a shell. A shared fragment would have to be downloaded
too, turning a one-request install into two and creating a version-skew failure
mode between the pattern and the installer that fetched it. The duplication is
one regex in two languages, and both are covered by tests that would fail
independently.

## 3. Why `select_stable_tag` reads stdin

`get_version` already holds the fetched page in a shell variable, so the selector
could take it as an argument. Reading stdin instead keeps the function a filter:
bats can feed it a heredoc fixture with no quoting hazard, and a page containing
arbitrary HTML never has to survive being passed through `"$1"` in a shell that
also has to survive `set -e`. The cost is one `printf '%s'` at the call site.

## 4. The 250-line ceiling

`install.bats` asserts `install.sh` is under 250 lines, and the file is at 227.
The additions are a pattern constant, a two-line predicate, a four-line selector
and two edited lines in `get_version` — roughly ten lines net including the
comments explaining *why* the guard exists. The ceiling stays a real constraint
rather than something this change quietly raises; if a future addition needs the
room, raising it is its own decision.

## 5. What `set -e` forces

`install.sh` runs under `set -e`, which shapes the implementation twice:

- `grep` returning 1 on no-match must never be the last command of an unguarded
  pipeline. `select_stable_tag` ends in `head -1`, which exits 0 whether or not
  the grep before it matched, so "no stable tag on the page" arrives as an empty
  string — the condition `get_version` already tests — rather than as an abort.
- `is_stable_version` is only ever used as the condition of an `if`, where a
  non-zero status is data rather than an error. Using it as a bare statement would
  abort the script on a prerelease, which is why it is a predicate and not a
  validator that exits.
