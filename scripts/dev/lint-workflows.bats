#!/usr/bin/env bats
#
# Fixture tests for lint-workflows.sh. Every violating case is a temp directory
# holding a minimal workflow, reached through the script's optional directory
# argument — the alternative would be committing a release workflow that
# releases from two branches to prove the check notices.

LINT() { "${BATS_TEST_DIRNAME}/lint-workflows.sh" "$@"; }

setup() {
  FIXTURE="$(mktemp -d)"
}

teardown() {
  rm -rf "$FIXTURE"
}

# A release workflow that satisfies every invariant. $1, when given, replaces
# the `branches: [main]` line so invariant-1 cases can vary just that.
write_cd() {
  cat > "$FIXTURE/cd.yml" <<YAML
name: Release

on:
  workflow_dispatch:
  push:
    ${1:-branches: [main]}

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: cargo build --release
YAML
}

# --- The committed tree ------------------------------------------------------

@test "the repository's own workflows satisfy every invariant" {
  run LINT
  [ "$status" -eq 0 ]
  [[ "$output" == *"All release-delivery invariants hold."* ]]
}

@test "the script is executable and shellcheck-clean shape (has a bash shebang)" {
  [ -x "${BATS_TEST_DIRNAME}/lint-workflows.sh" ]
  head -1 "${BATS_TEST_DIRNAME}/lint-workflows.sh" | grep -q 'bash'
}

# --- Invariant 1: the push trigger ------------------------------------------

@test "invariant 1 passes for the flow list form" {
  write_cd 'branches: [main]'
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}

@test "invariant 1 passes for the block list form" {
  cat > "$FIXTURE/cd.yml" <<'YAML'
name: Release

on:
  push:
    branches:
      - main
YAML
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}

@test "invariant 1 fails on a second branch" {
  write_cd 'branches: [main, v2]'
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"names \`v2\`"* ]]
}

@test "invariant 1 fails on a branch that is not main" {
  write_cd 'branches: [release]'
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"names \`release\`"* ]]
}

@test "invariant 1 fails on a tags key beside the branch list" {
  cat > "$FIXTURE/cd.yml" <<'YAML'
name: Release

on:
  push:
    branches: [main]
    tags: ['v*']
YAML
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *'`tags:`'* ]]
}

@test "invariant 1 fails when the push trigger is gone" {
  cat > "$FIXTURE/cd.yml" <<'YAML'
name: Release

on:
  workflow_dispatch:
YAML
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"no \`push:\` trigger"* ]]
}

@test "a missing cd.yml fails rather than passing" {
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not exist"* ]]
}

# --- Invariant 2: the plugin feature ----------------------------------------

@test "invariant 2 fails on --features plugins" {
  write_cd
  printf '      - run: cargo build --release --features plugins\n' >> "$FIXTURE/cd.yml"
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not build with the plugin feature"* ]]
}

@test "invariant 2 fails on --features=plugins" {
  write_cd
  printf '      - run: cargo build --features=plugins\n' >> "$FIXTURE/cd.yml"
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
}

@test "invariant 2 fails on --all-features" {
  write_cd
  printf '      - run: cargo build --release --all-features\n' >> "$FIXTURE/cd.yml"
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
}

@test "invariant 2 fails on a manifest edit adding the feature" {
  write_cd
  printf '      - run: sed -i '\''s/^/features = ["plugins"]/'\'' Cargo.toml\n' >> "$FIXTURE/cd.yml"
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
}

@test "invariant 2 tolerates a comment naming the feature it forbids" {
  write_cd
  printf '      # Never add --features plugins here: see RELEASE invariant 2.\n' >> "$FIXTURE/cd.yml"
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}

# --- Invariant 3: no package channels from nightly.yml ----------------------

@test "invariants 3 and 4 are not applicable without a nightly workflow" {
  write_cd
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"invariant 3"*"not applicable"* ]]
  [[ "$output" == *"invariant 4"*"not applicable"* ]]
}

@test "a nightly workflow with no release step and no channel passes" {
  write_cd
  cat > "$FIXTURE/nightly.yml" <<'YAML'
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: cargo build --release --features plugins
YAML
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}

@test "invariant 3 fails on a channel publish job" {
  write_cd
  cat > "$FIXTURE/nightly.yml" <<'YAML'
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  publish-chocolatey:
    runs-on: windows-latest
    steps:
      - run: echo hi
YAML
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"channel publish job"* ]]
}

@test "invariant 3 fails on any of the four channel credentials" {
  for secret in CHOCOLATEY_API_KEY WINGET_TOKEN HOMEBREW_TAP_DEPLOY_KEY AUR_SSH_PRIVATE_KEY; do
    write_cd
    cat > "$FIXTURE/nightly.yml" <<YAML
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  ship:
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
        env:
          KEY: \${{ secrets.$secret }}
YAML
    run LINT "$FIXTURE"
    [ "$status" -ne 0 ]
    [[ "$output" == *"channel credential"* ]]
  done
}

@test "invariant 3 fails on channel tooling" {
  for tool in "choco push thurbox.nupkg" "wingetcreate submit manifests"; do
    write_cd
    cat > "$FIXTURE/nightly.yml" <<YAML
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  ship:
    runs-on: ubuntu-latest
    steps:
      - run: $tool
YAML
    run LINT "$FIXTURE"
    [ "$status" -ne 0 ]
    [[ "$output" == *"channel tooling"* ]]
  done
}

# --- Invariant 4: nightly releases are prereleases --------------------------

# A nightly workflow whose single release step's `prerelease:` line is $1.
write_nightly_release() {
  cat > "$FIXTURE/nightly.yml" <<YAML
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - name: Create nightly release
        uses: softprops/action-gh-release@3d0d9888cb7fd7b750713d6e236d1fcb99157228 # v3.0.2
        with:
          tag_name: nightly-2026-08-08
          files: dist/*
$1
YAML
}

@test "invariant 4 passes when the release step is marked prerelease" {
  write_cd
  write_nightly_release '          prerelease: true'
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}

@test "invariant 4 fails when the release step is marked prerelease false" {
  write_cd
  write_nightly_release '          prerelease: false'
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not marked prerelease"* ]]
}

@test "invariant 4 fails when the release step omits the flag" {
  write_cd
  write_nightly_release '          draft: false'
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"prerelease"* ]]
}

@test "invariant 4 fails on gh release create without --prerelease" {
  write_cd
  cat > "$FIXTURE/nightly.yml" <<'YAML'
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - run: gh release create nightly-2026-08-08 dist/*
YAML
  run LINT "$FIXTURE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"--prerelease"* ]]
}

@test "invariant 4 accepts --prerelease on a continuation line" {
  write_cd
  cat > "$FIXTURE/nightly.yml" <<'YAML'
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - run: |
          gh release create nightly-2026-08-08 \
            --prerelease \
            dist/*
YAML
  run LINT "$FIXTURE"
  [ "$status" -eq 0 ]
}
