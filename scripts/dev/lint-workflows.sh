#!/usr/bin/env bash
# Enforce the release-delivery invariants over the GitHub Actions workflows.
#
# v2 lands on `main` while v1 keeps releasing from the same trunk. Four
# properties of the workflow files keep those two facts from colliding, and each
# is one line away from being false, invisible in review (the diff looks like
# ordinary CI maintenance), and discovered only by a user who installed the wrong
# thing:
#
#   1. cd.yml's push trigger names `main` and nothing else.
#   2. cd.yml never builds without the plugin runtime (since Stage B).
#   3. No package-channel publish job runs from nightly.yml.
#   4. Every nightly GitHub release is marked prerelease.
#
# Invariants 3 and 4 constrain a nightly.yml that does not exist yet: they pass
# as `not applicable` so the check can land *before* the workflow it constrains,
# which is the point — a lint written alongside that workflow is a lint shaped to
# pass it.
#
# Usage: lint-workflows.sh [WORKFLOW_DIR]
# The directory argument exists so scripts/dev/lint-workflows.bats can exercise
# every violating case against fixtures instead of committing a broken workflow.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORKFLOWS="${1:-$ROOT/.github/workflows}"
CD="$WORKFLOWS/cd.yml"
NIGHTLY="$WORKFLOWS/nightly.yml"

FAILED=0

fail() {
    printf 'lint-workflows: %s\n' "$1" >&2
    FAILED=1
}

# Reports each line of a captured `grep -n` result as one failure, prefixed with
# what kind of violation it is. Empty input reports nothing.
fail_each() {
    local prefix="$1" hits="$2" line
    [ -n "$hits" ] || return 0
    while IFS= read -r line; do
        [ -n "$line" ] && fail "$prefix: ${line}"
    done <<<"$hits"
}

# --- Invariant 1: cd.yml's push trigger names `main` and nothing else --------
#
# Structural, not a grep: "and nothing else" is a claim about the whole `push`
# mapping, so `branches: [main]` sitting beside `tags: ['v*']` must fail — and a
# grep for the branch list would pass it. The walk understands the two-space
# nesting these workflows use plus both list forms; anything it cannot parse
# leaves the mapping unseen, which fails rather than passing silently.
invariant_push_trigger() {
    local name="invariant 1: cd.yml push trigger is \`main\` and nothing else"
    local before=$FAILED

    if [ ! -f "$CD" ]; then
        fail "$name — $CD does not exist"
        return 0
    fi

    # Emits `key <name>` per key of the push mapping and `branch <name>` per
    # entry of its branches list.
    local parsed keys branches bad_key branch
    parsed=$(awk '
        # Any top-level key ends the block we were inside.
        /^[^[:space:]#]/ { in_on = ($0 ~ /^"?on"?:/); in_push = 0; in_branches = 0; next }
        in_on && /^  [^[:space:]#]/ {
            in_push = ($0 ~ /^  "?push"?:[[:space:]]*$/)
            in_branches = 0
            next
        }
        in_push && /^    [^[:space:]#-]/ {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            key = line
            sub(/:.*$/, "", key)
            gsub(/"/, "", key)
            print "key " key
            in_branches = (key == "branches")
            if (in_branches) {
                value = line
                sub(/^[^:]*:[[:space:]]*/, "", value)
                if (value ~ /^\[/) {              # flow form: branches: [main]
                    gsub(/[][]/, "", value)
                    gsub(/[[:space:]"'\'']/, "", value)
                    n = split(value, parts, ",")
                    for (i = 1; i <= n; i++) {
                        if (parts[i] != "") print "branch " parts[i]
                    }
                }
            }
            next
        }
        in_push && in_branches && /^ +-[[:space:]]/ {   # block form: - main
            value = $0
            sub(/^[[:space:]]*-[[:space:]]*/, "", value)
            gsub(/["'\''[:space:]]/, "", value)
            print "branch " value
            next
        }
    ' "$CD")

    keys=$(printf '%s\n' "$parsed" | sed -n 's/^key //p')
    branches=$(printf '%s\n' "$parsed" | sed -n 's/^branch //p')

    if [ -z "$keys" ]; then
        fail "$name — no \`push:\` trigger found under \`on:\` in $CD"
        return 0
    fi

    for bad_key in $(printf '%s\n' "$keys" | grep -vx 'branches' || true); do
        fail "$name — \`push\` carries \`$bad_key:\`, which is not \`branches\`"
    done

    if [ -z "$branches" ]; then
        fail "$name — \`push.branches\` is empty in $CD"
        return 0
    fi

    for branch in $branches; do
        [ "$branch" = "main" ] || fail "$name — \`push.branches\` names \`$branch\`"
    done

    [ "$FAILED" -eq "$before" ] && printf '  ok  %s\n' "$name"
    return 0
}

# --- Invariant 2: cd.yml never builds without the plugin runtime ------------
#
# This invariant used to run the other way: it forbade cd.yml from asking for
# `--features plugins`, because a v1 release binary containing the v2 runtime
# voided the compile-time gate. Stage B put `plugins` in the crate's *default*
# feature set, which retires that property deliberately — and, worse, makes the
# old check dishonest: no release job asks for the feature any more, so the check
# would report `ok` while every release binary carried exactly what it claimed to
# forbid. A check that has stopped tracking its own property is worse than none,
# because green is trusted.
#
# The hazard reversed direction. Once a native pane is handed over to its bundled
# plugin, the runtime is what *draws* that pane, so a release built with default
# features suppressed ships an empty column where the pane was — the same silent
# failure, arrived at from the other side. Hence: no `--no-default-features`, and
# no manifest edit rewriting the default feature list.
#
# Asking for the feature explicitly is now allowed and merely redundant: it
# requests the runtime the release is required to carry.
#
# Whole-line comments are stripped first, so cd.yml may document this invariant
# without the documentation tripping it. Trailing comments are left alone: they
# cannot hide a violation, since the code before them is still scanned.
invariant_keeps_plugin_runtime() {
    local name='invariant 2: cd.yml does not build without the plugin runtime'
    local before=$FAILED hits

    if [ ! -f "$CD" ]; then
        fail "$name — $CD does not exist"
        return 0
    fi

    hits=$(grep -nE -- '--no-default-features|default[[:space:]]*=[[:space:]]*\[' "$CD" |
        grep -vE '^[0-9]+:[[:space:]]*#' || true)
    fail_each "$name — $CD drops it" "$hits"

    [ "$FAILED" -eq "$before" ] && printf '  ok  %s\n' "$name"
    return 0
}

# --- Invariant 3: no package-channel publish runs from nightly.yml ----------
#
# Chocolatey and winget are moderated channels: a nightly there burns a human
# review and feeds a queue that starts returning 403 when pushed too fast.
invariant_nightly_no_channels() {
    local name='invariant 3: nightly.yml publishes to no package channel'
    local before=$FAILED

    if [ ! -f "$NIGHTLY" ]; then
        printf '  --  %s (not applicable)\n' "$name"
        return 0
    fi

    fail_each "$name — channel publish job" \
        "$(grep -nE '^  publish-[a-z0-9-]*:' "$NIGHTLY" || true)"
    fail_each "$name — channel credential" \
        "$(grep -nE 'CHOCOLATEY_API_KEY|WINGET_TOKEN|HOMEBREW_TAP_DEPLOY_KEY|AUR_SSH_PRIVATE_KEY' "$NIGHTLY" || true)"
    fail_each "$name — channel tooling" \
        "$(grep -nE 'choco push|wingetcreate|homebrew-thurbox|deploy-aur' "$NIGHTLY" || true)"

    [ "$FAILED" -eq "$before" ] && printf '  ok  %s\n' "$name"
    return 0
}

# --- Invariant 4: every nightly release is marked prerelease ----------------
#
# An unmarked nightly becomes `releases/latest`, which every unpinned installer
# resolves to — including the API path the installers treat as trustworthy.
invariant_nightly_prerelease() {
    local name='invariant 4: every nightly release is marked prerelease'
    local before=$FAILED

    if [ ! -f "$NIGHTLY" ]; then
        printf '  --  %s (not applicable)\n' "$name"
        return 0
    fi

    # A release-action step is a `-` list item whose block uses the release
    # action; the block must also carry `prerelease: true`. A `gh release create`
    # must carry `--prerelease` within its own run block, which YAML line
    # continuations let it reach on a later line.
    fail_each "$name" "$(awk '
        function flush_step() {
            if (is_release && !marked) {
                printf "%d: release step \"%s\" is not marked `prerelease: true`\n", start, label
            }
            is_release = 0; marked = 0; label = ""; start = 0
        }
        # A step boundary: a `-` list item at the indent the current step opened
        # at, or shallower.
        /^[[:space:]]*-[[:space:]]/ {
            match($0, /^[[:space:]]*/)
            indent = RLENGTH
            if (start && indent <= step_indent) flush_step()
            if (!start) { start = NR; step_indent = indent }
        }
        /^[[:space:]]*[-[:space:]]*name:[[:space:]]*/ {
            if (start && label == "") { label = $0; sub(/^.*name:[[:space:]]*/, "", label) }
        }
        /uses:[[:space:]]*softprops\/action-gh-release/ { is_release = 1 }
        /^[[:space:]]*prerelease:[[:space:]]*true[[:space:]]*$/ { marked = 1 }
        /^[[:space:]]*prerelease:[[:space:]]*/ {
            if ($0 !~ /:[[:space:]]*true[[:space:]]*$/) {
                printf "%d: release is not marked prerelease:%s\n", NR, $0
                # Reported here, so the step-level check does not say it twice.
                marked = 1
            }
        }
        # A `gh release create` and the five lines that may continue it.
        /gh release create/ { pending = 5; pending_line = NR; buf = $0; next }
        pending > 0 { buf = buf " " $0; pending-- }
        pending == 0 && pending_line > 0 {
            if (buf !~ /--prerelease/) {
                printf "%d: `gh release create` without `--prerelease`\n", pending_line
            }
            pending_line = 0; buf = ""
        }
        END {
            flush_step()
            if (pending_line > 0 && buf !~ /--prerelease/) {
                printf "%d: `gh release create` without `--prerelease`\n", pending_line
            }
        }
    ' "$NIGHTLY")"

    [ "$FAILED" -eq "$before" ] && printf '  ok  %s\n' "$name"
    return 0
}

printf 'Checking release-delivery invariants in %s\n' "$WORKFLOWS"
invariant_push_trigger
invariant_keeps_plugin_runtime
invariant_nightly_no_channels
invariant_nightly_prerelease

if [ "$FAILED" -ne 0 ]; then
    printf '\nlint-workflows: FAILED. The rationale for each invariant is in this script'\''s header.\n' >&2
    exit 1
fi
printf 'All release-delivery invariants hold.\n'
