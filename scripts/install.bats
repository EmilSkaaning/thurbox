#!/usr/bin/env bats

@test "script has valid shell syntax" {
  sh -n "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script is executable" {
  [ -x "${BATS_TEST_DIRNAME}/install.sh" ]
}

@test "script has POSIX shebang" {
  head -1 "${BATS_TEST_DIRNAME}/install.sh" | grep -q "#!/usr/bin/env sh"
}

@test "script contains cleanup function" {
  grep -q "^cleanup()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains detect_platform function" {
  grep -q "^detect_platform()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains get_target function" {
  grep -q "^get_target()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains cmd_exists function" {
  grep -q "^cmd_exists()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains download function" {
  grep -q "^download()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains get_version function" {
  grep -q "^get_version()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains get_checksum function" {
  grep -q "^get_checksum()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains check_sum function" {
  grep -q "^check_sum()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains get_binary function" {
  grep -q "^get_binary()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains do_install function" {
  grep -q "^do_install()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains show_success function" {
  grep -q "^show_success()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script contains main function" {
  grep -q "^main()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script has trap cleanup" {
  grep -q "trap cleanup" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script is under 250 lines" {
  [ $(wc -l < "${BATS_TEST_DIRNAME}/install.sh") -lt 250 ]
}

@test "script contains banner function" {
  grep -q "^banner()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "banner renders ASCII art to stderr" {
  run sh -c "TEST_TMPDIR=1 . '${BATS_TEST_DIRNAME}/install.sh'; banner" 2>&1
  [ "$status" -eq 0 ]
  [ -n "$output" ]
}

@test "colors disabled when NO_COLOR is set" {
  run sh -c "TEST_TMPDIR=1 NO_COLOR=1 . '${BATS_TEST_DIRNAME}/install.sh'; printf '%s' \"\$C_RED\""
  [ -z "$output" ]
}

@test "script has logging functions" {
  grep -q "^info()" "${BATS_TEST_DIRNAME}/install.sh"
  grep -q "^error()" "${BATS_TEST_DIRNAME}/install.sh"
  grep -q "^success()" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script maps linux-x86_64 to musl" {
  grep -q "linux-x86_64) echo \"x86_64-unknown-linux-musl\"" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script maps darwin-aarch64 correctly" {
  grep -q "darwin-aarch64) echo \"aarch64-apple-darwin\"" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script does not map unshipped platforms (linux-aarch64, darwin-x86_64)" {
  # cd.yml builds no .tar.gz for these, so get_target must NOT resolve them.
  run grep -E "linux-aarch64\)|darwin-x86_64\)" "${BATS_TEST_DIRNAME}/install.sh"
  [ "$status" -ne 0 ]
}

@test "script supports VERSION env var" {
  grep -q "VERSION" "${BATS_TEST_DIRNAME}/install.sh"
}

@test "script supports INSTALL_DIR env var" {
  grep -q "INSTALL_DIR" "${BATS_TEST_DIRNAME}/install.sh"
}

# --- Stable-only version resolution -----------------------------------------
# Nightlies ship as GitHub prereleases and the releases page lists them, so a
# guessed version that accepted a prerelease tag would install a v2 nightly to
# someone who asked for stable. These pin that it cannot.

# Emits a releases page whose linked tags are the arguments, in order. Shaped
# like GitHub's markup so the scrape is exercised as it is in production.
releases_page() {
  for tag in "$@"; do
    printf '<a href="/Thurbeen/thurbox/releases/tag/%s">%s</a>\n' "$tag" "$tag"
  done
}

@test "is_stable_version accepts a plain three-part version" {
  TEST_TMPDIR=1 sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    is_stable_version v1.4.12 && is_stable_version v10.0.1"
}

@test "is_stable_version refuses a prerelease tag" {
  run env TEST_TMPDIR=1 sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    is_stable_version v2.0.0-nightly.20260808"
  [ "$status" -ne 0 ]
}

@test "is_stable_version refuses near-miss shapes" {
  for tag in v1.2 v1.2.3.4 1.2.3 v1.2.3+build5 v1.2.3-rc.1; do
    run env TEST_TMPDIR=1 sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
      is_stable_version '$tag'"
    [ "$status" -ne 0 ]
  done
}

@test "select_stable_tag skips a prerelease listed above a stable release" {
  PAGE=$(releases_page v2.0.0-nightly.20260808 v1.4.12)
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    printf '%s\n' \"\$PAGE\" | select_stable_tag"
  [ "$status" -eq 0 ]
  [ "$output" = "v1.4.12" ]
}

@test "select_stable_tag skips several prereleases" {
  PAGE=$(releases_page v2.0.0-nightly.20260809 v2.0.0-nightly.20260808 v2.0.0-rc.1 v1.4.12)
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    printf '%s\n' \"\$PAGE\" | select_stable_tag"
  [ "$output" = "v1.4.12" ]
}

@test "select_stable_tag takes the newest tag when it is stable" {
  PAGE=$(releases_page v1.5.0 v1.4.12)
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    printf '%s\n' \"\$PAGE\" | select_stable_tag"
  [ "$output" = "v1.5.0" ]
}

@test "select_stable_tag returns nothing for a prerelease-only page" {
  PAGE=$(releases_page v2.0.0-nightly.20260808 v2.0.0-rc.1)
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    printf '%s\n' \"\$PAGE\" | select_stable_tag"
  [ -z "$output" ]
}

@test "get_version resolves stable when the API fails and the page leads with a prerelease" {
  PAGE=$(releases_page v2.0.0-nightly.20260808 v1.4.12)
  # curl -s swallows a 4xx, so a rate-limited API returns a body with no
  # tag_name and the scrape runs - the routine path, not an exotic one.
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    fetch_url() { case \"\$1\" in
      *api.github.com*) printf '{\"message\": \"API rate limit exceeded\"}\n' ;;
      *) printf '%s\n' \"\$PAGE\" ;;
    esac; }
    get_version"
  [ "$status" -eq 0 ]
  [ "$output" = "v1.4.12" ]
}

@test "get_version refuses a prerelease returned by the API" {
  PAGE=$(releases_page v1.4.12)
  run env TEST_TMPDIR=1 PAGE="$PAGE" sh -c ". '${BATS_TEST_DIRNAME}/install.sh'
    fetch_url() { case \"\$1\" in
      *api.github.com*) printf '{\"tag_name\": \"v2.0.0-nightly.20260808\"}\n' ;;
      *) printf '%s\n' \"\$PAGE\" ;;
    esac; }
    get_version"
  [ "$status" -eq 0 ]
  [ "$output" = "v1.4.12" ]
}

@test "get_version passes a pinned prerelease through untouched" {
  run env TEST_TMPDIR=1 VERSION=v2.0.0-nightly.20260808 sh -c \
    ". '${BATS_TEST_DIRNAME}/install.sh'
     fetch_url() { echo 'fetch_url must not be called'; exit 9; }
     get_version"
  [ "$status" -eq 0 ]
  [ "$output" = "v2.0.0-nightly.20260808" ]
}

@test "do_install succeeds with only the thurbox binary in tarball" {
  tmpdir=$(mktemp -d)
  mkdir -p "$tmpdir/src" "$tmpdir/dest"
  printf '#!/bin/sh\n' > "$tmpdir/src/thurbox"
  tar -czf "$tmpdir/archive.tar.gz" -C "$tmpdir/src" thurbox
  TEST_TMPDIR=1 sh -c ". '${BATS_TEST_DIRNAME}/install.sh'; do_install '$tmpdir/archive.tar.gz' '$tmpdir/dest'"
  [ -x "$tmpdir/dest/thurbox" ]
  rm -rf "$tmpdir"
}

@test "do_install chmods both binaries when thurbox-cli is present" {
  tmpdir=$(mktemp -d)
  mkdir -p "$tmpdir/src" "$tmpdir/dest"
  printf '#!/bin/sh\n' > "$tmpdir/src/thurbox"
  printf '#!/bin/sh\n' > "$tmpdir/src/thurbox-cli"
  tar -czf "$tmpdir/archive.tar.gz" -C "$tmpdir/src" thurbox thurbox-cli
  TEST_TMPDIR=1 sh -c ". '${BATS_TEST_DIRNAME}/install.sh'; do_install '$tmpdir/archive.tar.gz' '$tmpdir/dest'"
  [ -x "$tmpdir/dest/thurbox" ]
  [ -x "$tmpdir/dest/thurbox-cli" ]
  rm -rf "$tmpdir"
}

@test "script conditionally chmods thurbox-cli" {
  grep -q 'thurbox-cli.*chmod' "${BATS_TEST_DIRNAME}/install.sh"
}
